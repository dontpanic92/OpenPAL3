//! Phase 5 (D1/D2/D3): production [`ImmediateDirectorPump`] backed
//! by `imgui-rs`. Installed on `CoreRadianceEngine` via
//! [`radiance::radiance::CoreRadianceEngine::set_immediate_director_pump`].
//!
//! Per-frame work:
//! 1. QI the supplied director to
//!    [`IImmediateDirector`](crate::comdef::immediate_director::IImmediateDirector).
//!    Silent skip if the active director is not immediate-mode.
//! 2. Park an [`ImguiFrameState`](crate::services::ui_host::ImguiFrameState)
//!    around the script call so `ImguiUiHost` can issue real imgui-rs
//!    calls.
//! 3. Invoke `render_im(ui_host, dt)` on the QI'd
//!    `ComRc<IImmediateDirector>`.
//!
//! The texture cache lives inside this pump for Phase 5; Phase E
//! may move it onto `UiManager` itself.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::IDirector;
use radiance::radiance::{ImmediateDirectorPump, UiManager};

use crate::comdef::immediate_director::{IImmediateDirector, IUiHost};
use crate::services::ui_host::{with_imgui_frame, ImguiFrameState};
use crate::services::ImguiTextureCache;

pub struct ImguiImmediateDirectorPump {
    ui_manager: Rc<UiManager>,
    textures: Rc<RefCell<ImguiTextureCache>>,
    ui_host: ComRc<IUiHost>,
}

impl ImguiImmediateDirectorPump {
    pub fn new(
        ui_manager: Rc<UiManager>,
        textures: Rc<RefCell<ImguiTextureCache>>,
        ui_host: ComRc<IUiHost>,
    ) -> Self {
        Self {
            ui_manager,
            textures,
            ui_host,
        }
    }

    /// Headless dispatch: QI + render_im invocation without parking
    /// an imgui frame state. Useful for tests that drive the pump
    /// with a recording `IUiHost` and do not need a real imgui
    /// context. Returns `true` if the QI succeeded and `render_im`
    /// was called.
    pub fn dispatch_render_im(
        director: &ComRc<IDirector>,
        ui_host: &ComRc<IUiHost>,
        dt: f32,
    ) -> bool {
        let Some(im) = director.query_interface::<IImmediateDirector>() else {
            return false;
        };
        im.render_im(ui_host.clone(), dt);
        true
    }
}

impl ImmediateDirectorPump for ImguiImmediateDirectorPump {
    fn pump(&self, director: ComRc<IDirector>, dt: f32) {
        let Some(im) = director.query_interface::<IImmediateDirector>() else {
            return;
        };
        let ui = self.ui_manager.ui();
        let fonts: Vec<imgui::FontId> = ui.fonts().fonts().to_vec();
        let mut tex = self.textures.borrow_mut();
        let mut frame =
            ImguiFrameState::new(ui, &mut *tex, &fonts, self.ui_manager.dpi_scale());
        with_imgui_frame(&mut frame, || {
            im.render_im(self.ui_host.clone(), dt);
        });
    }
}
