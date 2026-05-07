use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use crosscom_protosept::HostError;
use p7::interpreter::context::Data;
use radiance::comdef::{IDirector, IDirectorImpl};
use radiance::radiance::UiManager;

use crate::comdef::services::IHostContext;
use crate::command_router::{dispatch_commands, CommandRouter, NullCommandRouter};
use crate::runtime::ScriptRuntime;
use crate::services::ImguiTextureCache;
use crate::ui_walker::{owned, walk, LocalCommandQueue, UiAdapter, WalkContext};

pub struct ScriptedDirector {
    runtime: Rc<RefCell<ScriptRuntime>>,
    ui_manager: Option<Rc<UiManager>>,
    textures: Option<Rc<RefCell<ImguiTextureCache>>>,
    router: Rc<dyn CommandRouter>,
}

ComObject_ScriptedDirector!(super::ScriptedDirector);

impl IDirectorImpl for ScriptedDirector {
    fn activate(&self) {
        let Some(state) = self.runtime.borrow().state_clone() else {
            return;
        };
        if !self.runtime.borrow().has_function("activate") {
            return;
        }
        if let Err(err) = self.runtime.borrow_mut().call_void("activate", vec![state]) {
            log::error!("scripted director activate failed: {}", err);
        }
    }

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        let state = self.runtime.borrow().state_clone()?;

        let owned = if self.ui_manager.is_some() && self.textures.is_some() {
            let render_args = vec![state.clone(), Data::Float(delta_sec as f64)];
            let mut rt = self.runtime.borrow_mut();
            match rt.call_returning_data("render", render_args) {
                Ok(node) => match rt.with_ctx(|ctx| owned::resolve(ctx, &node)) {
                    Ok(owned) => Some(owned),
                    Err(err) => {
                        log::error!("scripted director walker resolve failed: {}", err);
                        None
                    }
                },
                Err(err) => {
                    log::error!("scripted director render fn failed: {}", err);
                    None
                }
            }
        } else {
            None
        };

        if let (Some(owned), Some(ui_manager), Some(textures)) =
            (owned.as_ref(), self.ui_manager.as_ref(), self.textures.as_ref())
        {
            let ui = ui_manager.ui();
            let mut queue = LocalCommandQueue::default();
            let fonts: Vec<imgui::FontId> = ui.fonts().fonts().to_vec();
            {
                let mut tex = textures.borrow_mut();
                let mut adapter = UiAdapter {
                    ui,
                    ctx: WalkContext {
                        textures: &mut *tex,
                        commands: &mut queue,
                        fonts: &fonts,
                    },
                    table_counter: std::cell::Cell::new(0),
                };
                if let Err(err) = walk(owned, &mut adapter) {
                    log::error!("scripted director walk failed: {:?}", err);
                }
            }

            if !queue.queue.is_empty() {
                log::info!(
                    "scripted director: walk produced {} command(s): {:?}",
                    queue.queue.len(),
                    queue.queue
                );
            }
            if let Some(next) = dispatch_commands(&mut queue, self.router.as_ref()) {
                log::info!("scripted director: router returned next director, swapping");
                return Some(next);
            }
        }

        match self
            .runtime
            .borrow_mut()
            .call_returning_optional_com::<IDirector>(
                "update",
                vec![state, Data::Float(delta_sec as f64)],
            ) {
            Ok(next) => next,
            Err(err) => {
                log::error!("scripted director update failed: {}", err);
                None
            }
        }
    }
}

impl ScriptedDirector {
    pub fn create(
        source: &str,
        host_ctx: ComRc<IHostContext>,
    ) -> Result<ComRc<IDirector>, HostError> {
        let runtime = Self::init_runtime(source, host_ctx)?;
        Ok(ComRc::from_object(Self {
            runtime: Rc::new(RefCell::new(runtime)),
            ui_manager: None,
            textures: None,
            router: Rc::new(NullCommandRouter),
        }))
    }

    pub fn create_with_ui(
        source: &str,
        host_ctx: ComRc<IHostContext>,
        ui_manager: Rc<UiManager>,
        textures: Rc<RefCell<ImguiTextureCache>>,
        router: Rc<dyn CommandRouter>,
    ) -> Result<ComRc<IDirector>, HostError> {
        let runtime = Self::init_runtime(source, host_ctx)?;
        Ok(ComRc::from_object(Self {
            runtime: Rc::new(RefCell::new(runtime)),
            ui_manager: Some(ui_manager),
            textures: Some(textures),
            router,
        }))
    }

    fn init_runtime(
        source: &str,
        host_ctx: ComRc<IHostContext>,
    ) -> Result<ScriptRuntime, HostError> {
        let mut runtime = ScriptRuntime::new();
        runtime.load_source(source)?;
        let host_id = runtime.intern(host_ctx);
        let host =
            runtime.foreign_box("radiance_scripting.comdef.services.IHostContext", host_id)?;
        let state = runtime.call_returning_data("init", vec![host])?;
        runtime.store_state(state);
        Ok(runtime)
    }
}
