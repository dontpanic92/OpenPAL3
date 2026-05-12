use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crosscom::ComRc;
use crosscom_protosept::HostError;
use p7::interpreter::context::Data;
use radiance::radiance::UiManager;
use radiance_editor::comdef::{IViewContent, IViewContentImpl};

use crate::comdef::services::IHostContext;
use crate::runtime::ScriptRuntime;
use crate::services::texture_cache::ImguiTextureCache;
use crate::ui_walker::{owned, walk, LocalCommandQueue, UiAdapter, WalkContext};

pub struct ScriptedViewContent {
    runtime: Rc<RefCell<ScriptRuntime>>,
    ui_manager: Option<Rc<UiManager>>,
    textures: Option<Rc<RefCell<ImguiTextureCache>>>,
    pending_commands: Rc<RefCell<VecDeque<i32>>>,
}

ComObject_ScriptedViewContent!(super::ScriptedViewContent);

impl IViewContentImpl for ScriptedViewContent {
    fn render(&self, delta_sec: f32) {
        let Some(state) = self.runtime.borrow().state_clone() else {
            return;
        };

        if let (Some(ui_mgr), Some(tex_cache)) = (self.ui_manager.as_ref(), self.textures.as_ref())
        {
            let render_args = vec![state, Data::Float(delta_sec as f64)];
            let owned = {
                let mut rt = self.runtime.borrow_mut();
                match rt.call_returning_data("render", render_args) {
                    Ok(node) => match rt.with_ctx(|ctx| owned::resolve(ctx, &node)) {
                        Ok(owned) => Some(owned),
                        Err(err) => {
                            log::error!("scripted view content walker resolve failed: {}", err);
                            None
                        }
                    },
                    Err(err) => {
                        log::error!("scripted view content render fn failed: {}", err);
                        None
                    }
                }
            };

            if let Some(owned) = owned.as_ref() {
                let ui = ui_mgr.ui();
                let fonts: Vec<imgui::FontId> = ui.fonts().fonts().to_vec();
                let mut queue = LocalCommandQueue::default();
                {
                    let mut tex = tex_cache.borrow_mut();
                    let mut adapter = UiAdapter {
                        ui,
                        ctx: WalkContext {
                            textures: &mut *tex,
                            commands: &mut queue,
                            fonts: &fonts,
                            dpi_scale: ui_mgr.dpi_scale(),
                        },
                        table_counter: std::cell::Cell::new(0),
                    };
                    if let Err(err) = walk(owned, &mut adapter) {
                        log::error!("scripted view content walk failed: {:?}", err);
                    }
                }
                self.pending_commands.borrow_mut().extend(queue.queue);
            }
        } else if let Err(err) = self
            .runtime
            .borrow_mut()
            .call_void("render", vec![state, Data::Float(delta_sec as f64)])
        {
            log::error!("scripted view content render failed: {}", err);
        }
    }
}

impl ScriptedViewContent {
    pub fn create(
        source: &str,
        host_ctx: ComRc<IHostContext>,
    ) -> Result<ComRc<IViewContent>, HostError> {
        Self::create_inner(source, host_ctx, None, None)
    }

    pub fn create_with_ui(
        source: &str,
        host_ctx: ComRc<IHostContext>,
        ui_manager: Rc<UiManager>,
        textures: Rc<RefCell<ImguiTextureCache>>,
    ) -> Result<ComRc<IViewContent>, HostError> {
        Self::create_inner(source, host_ctx, Some(ui_manager), Some(textures))
    }

    pub fn drain_commands(&self) -> Vec<i32> {
        self.pending_commands.borrow_mut().drain(..).collect()
    }

    pub fn from_view(view: &ComRc<IViewContent>) -> &Self {
        let inner_offset = crosscom::offset_of!(
            ScriptedViewContent_crosscom_impl::ScriptedViewContentCcw,
            inner
        );
        unsafe {
            &*((view.ptr_value() as *const u8).add(inner_offset) as *const ScriptedViewContent)
        }
    }

    pub fn drain_commands_from_view(view: &ComRc<IViewContent>) -> Vec<i32> {
        Self::from_view(view).drain_commands()
    }

    fn create_inner(
        source: &str,
        host_ctx: ComRc<IHostContext>,
        ui_manager: Option<Rc<UiManager>>,
        textures: Option<Rc<RefCell<ImguiTextureCache>>>,
    ) -> Result<ComRc<IViewContent>, HostError> {
        let mut runtime = ScriptRuntime::new();
        runtime.load_source(source)?;
        let host_id = runtime.intern(host_ctx);
        let host =
            runtime.foreign_box("radiance_scripting.comdef.services.IHostContext", host_id)?;
        let state = runtime.call_returning_data("init", vec![host])?;
        runtime.store_state(state);
        Ok(ComRc::from_object(Self {
            runtime: Rc::new(RefCell::new(runtime)),
            ui_manager,
            textures,
            pending_commands: Rc::new(RefCell::new(VecDeque::new())),
        }))
    }
}
