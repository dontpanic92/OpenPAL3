use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use crosscom_protosept::HostError;
use p7::interpreter::context::Data;
use radiance::comdef::{IDirector, IDirectorImpl};
use radiance::radiance::UiManager;

use crate::runtime::{ScriptDirectorHandle, ScriptHost};
use crate::services::ImguiTextureCache;
use crate::ui_walker::{owned, walk, LocalCommandQueue, UiAdapter, WalkContext};

/// Rust-side `IDirector` ComObject that drives a single script-owned
/// `box<director.Director>` value. Every transition decision (which director
/// to run next) comes from the script via return values from `dispatch` or
/// `update`.
pub struct ScriptedDirector {
    host: Rc<ScriptHost>,
    handle: ScriptDirectorHandle,
    ui_manager: Option<Rc<UiManager>>,
    textures: Option<Rc<RefCell<ImguiTextureCache>>>,
}

ComObject_ScriptedDirector!(super::ScriptedDirector);

impl ScriptedDirector {
    /// Wraps a rooted script director handle as an `IDirector` ComObject for
    /// directors that do not render UI (e.g. test stubs).
    pub fn wrap(host: Rc<ScriptHost>, handle: ScriptDirectorHandle) -> ComRc<IDirector> {
        ComRc::from_object(Self {
            host,
            handle,
            ui_manager: None,
            textures: None,
        })
    }

    /// Wraps a rooted script director and equips it with the host's UI
    /// manager and texture cache so its `render` output drives imgui every
    /// frame.
    pub fn with_ui(
        host: Rc<ScriptHost>,
        handle: ScriptDirectorHandle,
        ui_manager: Rc<UiManager>,
        textures: Rc<RefCell<ImguiTextureCache>>,
    ) -> ComRc<IDirector> {
        ComRc::from_object(Self {
            host,
            handle,
            ui_manager: Some(ui_manager),
            textures: Some(textures),
        })
    }

    fn current_director(&self) -> Option<Data> {
        self.host.deref_handle(self.handle)
    }

    fn wrap_next(&self, director: Data) -> ComRc<IDirector> {
        let handle = self.host.root(director);
        ComRc::from_object(Self {
            host: self.host.clone(),
            handle,
            ui_manager: self.ui_manager.clone(),
            textures: self.textures.clone(),
        })
    }

    fn dispatch_script_command(
        &self,
        director: &Data,
        command_id: i32,
    ) -> Option<ComRc<IDirector>> {
        match self.host.call_method_returning_data(
            director.clone(),
            "dispatch",
            vec![Data::Int(command_id as i64)],
        ) {
            Ok(next) => match optional_script_director(next) {
                Ok(Some(next)) => Some(self.wrap_next(next)),
                Ok(None) => None,
                Err(err) => {
                    log::error!("scripted director dispatch returned invalid value: {}", err);
                    None
                }
            },
            Err(err) => {
                log::error!("scripted director dispatch failed: {}", err);
                None
            }
        }
    }
}

impl IDirectorImpl for ScriptedDirector {
    fn activate(&self) {
        let Some(director) = self.current_director() else {
            return;
        };
        if let Err(err) = self
            .host
            .call_method_void(director, "activate", Vec::new())
        {
            log::error!("scripted director activate failed: {}", err);
        }
    }

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        let director = self.current_director()?;

        let owned = if self.ui_manager.is_some() && self.textures.is_some() {
            let node = self.host.call_method_returning_data(
                director.clone(),
                "render",
                vec![Data::Float(delta_sec as f64)],
            );
            match node {
                Ok(node) => match self.host.with_ctx(|ctx| owned::resolve(ctx, &node)) {
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

        if let (Some(owned), Some(ui_manager), Some(textures)) = (
            owned.as_ref(),
            self.ui_manager.as_ref(),
            self.textures.as_ref(),
        ) {
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
                        dpi_scale: ui_manager.dpi_scale(),
                    },
                    table_counter: std::cell::Cell::new(0),
                };
                if let Err(err) = walk(owned, &mut adapter) {
                    log::error!("scripted director walk failed: {:?}", err);
                }
            }

            while let Some(command_id) = queue.queue.pop_front() {
                if let Some(next) = self.dispatch_script_command(&director, command_id) {
                    return Some(next);
                }
            }
        }

        match self.host.call_method_returning_data(
            director,
            "update",
            vec![Data::Float(delta_sec as f64)],
        ) {
            Ok(next) => match optional_script_director(next) {
                Ok(Some(next)) => Some(self.wrap_next(next)),
                Ok(None) => None,
                Err(err) => {
                    log::error!("scripted director update returned invalid value: {}", err);
                    None
                }
            },
            Err(err) => {
                log::error!("scripted director update failed: {}", err);
                None
            }
        }
    }
}

impl Drop for ScriptedDirector {
    fn drop(&mut self) {
        let Some(director) = self.host.deref_handle(self.handle) else {
            return;
        };
        if let Err(err) = self
            .host
            .call_method_void(director, "deactivate", Vec::new())
        {
            log::error!("scripted director deactivate failed: {}", err);
        }
        self.host.unroot(self.handle);
    }
}

fn optional_script_director(data: Data) -> Result<Option<Data>, HostError> {
    match data {
        Data::Null => Ok(None),
        Data::Some(inner) => Ok(Some(*inner)),
        Data::Array(mut values) => match values.len() {
            0 => Ok(None),
            1 => Ok(Some(values.remove(0))),
            len => Err(HostError::message(format!(
                "expected zero or one script director, got {len}"
            ))),
        },
        Data::BoxRef(_) | Data::ProtoBoxRef { .. } => Ok(Some(data)),
        other => Err(HostError::message(format!(
            "expected optional script director box, got {:?}",
            other
        ))),
    }
}
