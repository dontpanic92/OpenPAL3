//! `ScriptedImmediateDirector` — `IDirector` ComObject proxy for
//! script structs conforming to `director.ImmediateDirector` (the
//! canonical immediate-mode UI proto, introduced in H3 of
//! generated/plan.md and now the only director proto since H5
//! retired the retained `UiNode` path).
//!
//! The proxy:
//!
//! 1. Holds a singleton `ComRc<IUiHost>` (production: `ImguiUiHost`;
//!    tests: `RecordingUiHost`) interned in the host's `ComObjectTable`.
//! 2. On each `update`, materialises a fresh `box<IUiHost>` Data
//!    pointing at that ComObject and calls the script's
//!    `render_im(ui, dt)` method. The frame state (real imgui::Ui +
//!    texture resolver + fonts) must already be parked via
//!    [`crate::services::ui_host::with_imgui_frame`] when this proxy
//!    drives an `ImguiUiHost`; for a recording host no frame setup is
//!    needed.
//! 3. Pipes `update(dt) -> array<box<ImmediateDirector>>` through an
//!    `optional_immediate_director` normaliser that accepts a
//!    zero-or-one-element array, an `Option`, or a bare director box.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use crosscom_protosept::HostError;
use p7::interpreter::context::Data;
use radiance::comdef::{IDirector, IDirectorImpl};
use radiance::radiance::UiManager;

use crate::comdef::immediate_director::IUiHost;
use crate::runtime::{ScriptDirectorHandle, ScriptHost};
use crate::services::ui_host::{with_imgui_frame, ImguiFrameState};
use crate::services::ImguiTextureCache;

/// `IDirector` proxy that drives a script-side `box<ImmediateDirector>`.
pub struct ScriptedImmediateDirector {
    host: Rc<ScriptHost>,
    handle: ScriptDirectorHandle,
    /// Interned `ComRc<IUiHost>` handle, encoded as a
    /// `ComObjectTable` id. The actual `ComObject` lives in the
    /// host's services; we just rehydrate a fresh `box<IUiHost>`
    /// `Data` value each frame.
    ui_host_com_id: i64,
    /// Engine UI manager + texture cache used to park an
    /// `ImguiFrameState` for the `ImguiUiHost` impl. When `None`, the
    /// `IUiHost` ComObject is expected to be a host-controlled
    /// stub (e.g. `RecordingUiHost` in smoke tests) that does not
    /// depend on imgui frame state.
    ui_manager: Option<Rc<UiManager>>,
    textures: Option<Rc<RefCell<ImguiTextureCache>>>,
}

ComObject_ScriptedDirector!(super::ScriptedImmediateDirector);

/// p7 type-tag for `crosscom.IUiHost` carriers (matches the
/// `@foreign(type_tag=...)` declared in the generated `immediate_director.p7`).
const UI_HOST_TYPE_TAG: &str = "radiance_scripting.comdef.immediate_director.IUiHost";

impl ScriptedImmediateDirector {
    /// Wrap a rooted script-side `box<ImmediateDirector>` together
    /// with the `IUiHost` ComObject the host will pass to its
    /// `render_im` on every frame. No imgui frame state is parked —
    /// suitable for tests using a stub `IUiHost` (e.g.
    /// `RecordingUiHost`).
    pub fn wrap(
        host: Rc<ScriptHost>,
        handle: ScriptDirectorHandle,
        ui_host: ComRc<IUiHost>,
    ) -> ComRc<IDirector> {
        let ui_host_com_id = host.intern(ui_host);
        ComRc::from_object(Self {
            host,
            handle,
            ui_host_com_id,
            ui_manager: None,
            textures: None,
        })
    }

    /// Production wrap: same as [`Self::wrap`] but additionally parks
    /// an `ImguiFrameState` (real imgui::Ui + texture cache + fonts)
    /// around each `render_im` call so an `ImguiUiHost`-backed
    /// `IUiHost` can issue imgui-rs calls directly.
    pub fn with_ui(
        host: Rc<ScriptHost>,
        handle: ScriptDirectorHandle,
        ui_host: ComRc<IUiHost>,
        ui_manager: Rc<UiManager>,
        textures: Rc<RefCell<ImguiTextureCache>>,
    ) -> ComRc<IDirector> {
        let ui_host_com_id = host.intern(ui_host);
        ComRc::from_object(Self {
            host,
            handle,
            ui_host_com_id,
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
            ui_host_com_id: self.ui_host_com_id,
            ui_manager: self.ui_manager.clone(),
            textures: self.textures.clone(),
        })
    }

    /// Build a fresh `Data::ProtoBoxRef` wrapping the interned
    /// `IUiHost` ComObject for handoff to a single script render call.
    /// Allocated on the box heap per frame; collected at GC after the
    /// frame returns and the script no longer holds any reference.
    fn make_ui_box(&self) -> Result<Data, HostError> {
        self.host
            .foreign_box(UI_HOST_TYPE_TAG, self.ui_host_com_id)
            .map_err(|e| HostError::message(format!("make_ui_box: {e}")))
    }
}

impl IDirectorImpl for ScriptedImmediateDirector {
    fn activate(&self) {
        let Some(director) = self.current_director() else {
            return;
        };
        if let Err(err) = self
            .host
            .call_method_void(director, "activate", Vec::new())
        {
            log::error!("ImmediateDirector::activate failed: {err}");
        }
    }

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        let director = self.current_director()?;

        // Render pass: materialise `box<IUiHost>` and invoke
        // `render_im`. In production (`with_ui`), an `ImguiFrameState`
        // is parked via `with_imgui_frame` so the `ImguiUiHost` impl
        // can issue real imgui-rs calls. For test fixtures (recording
        // IUiHost via `wrap`), no frame state is needed.
        match self.make_ui_box() {
            Ok(ui_box) => {
                let render_call = || {
                    if let Err(err) = self.host.call_method_void(
                        director.clone(),
                        "render_im",
                        vec![ui_box, Data::Float(delta_sec as f64)],
                    ) {
                        log::error!("ImmediateDirector::render_im failed: {err}");
                    }
                };
                match (self.ui_manager.as_ref(), self.textures.as_ref()) {
                    (Some(ui_manager), Some(textures)) => {
                        let ui = ui_manager.ui();
                        let fonts: Vec<imgui::FontId> = ui.fonts().fonts().to_vec();
                        let mut tex = textures.borrow_mut();
                        let mut frame = ImguiFrameState::new(
                            ui,
                            &mut *tex,
                            &fonts,
                            ui_manager.dpi_scale(),
                        );
                        with_imgui_frame(&mut frame, render_call);
                    }
                    _ => render_call(),
                }
            }
            Err(err) => {
                log::error!("ImmediateDirector::render_im setup failed: {err}");
            }
        }

        // Update pass: same transition shape as the legacy proxy.
        match self.host.call_method_returning_data(
            director,
            "update",
            vec![Data::Float(delta_sec as f64)],
        ) {
            Ok(next) => match optional_immediate_director(next) {
                Ok(Some(next)) => Some(self.wrap_next(next)),
                Ok(None) => None,
                Err(err) => {
                    log::error!("ImmediateDirector::update returned invalid value: {err}");
                    None
                }
            },
            Err(err) => {
                log::error!("ImmediateDirector::update failed: {err}");
                None
            }
        }
    }
}

impl Drop for ScriptedImmediateDirector {
    fn drop(&mut self) {
        let director = self.host.deref_handle(self.handle);
        if let Some(director) = director {
            if let Err(err) = self
                .host
                .call_method_void(director, "deactivate", Vec::new())
            {
                log::error!("ImmediateDirector::deactivate failed: {err}");
            }
        }
        self.host.unroot(self.handle);
        // Note: we intentionally do not release the `ui_host_com_id`
        // here. Multiple `ScriptedImmediateDirector` instances created
        // by transitions (`wrap_next`) share the same id, and the
        // host's `ComObjectTable` keeps the underlying ComObject alive
        // until the session ends.
    }
}

fn optional_immediate_director(data: Data) -> Result<Option<Data>, HostError> {
    match data {
        Data::Null => Ok(None),
        Data::Some(inner) => Ok(Some((*inner).clone())),
        Data::Array(values) => match values.len() {
            0 => Ok(None),
            1 => Ok(Some(values[0].clone())),
            len => Err(HostError::message(format!(
                "expected zero or one script immediate-director, got {len}"
            ))),
        },
        Data::BoxRef { .. } | Data::ProtoBoxRef { .. } => Ok(Some(data)),
        other => Err(HostError::message(format!(
            "expected optional script immediate-director box, got {:?}",
            other
        ))),
    }
}
