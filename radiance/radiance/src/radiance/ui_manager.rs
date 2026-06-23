use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use imgui::Ui;

use crate::{
    application::Platform,
    comdef::IUiLayer,
    imgui::{ImguiContext, ImguiFrame, TextureResolver},
    radiance::ui_layer::{self, UiLayerBand, UiLayerHandle, UiLayerStack},
};

pub struct UiManager {
    imgui_context: Rc<ImguiContext>,
    ui: RefCell<Option<&'static Ui>>,
    dpi_scale: f32,
    layers: Rc<RefCell<UiLayerStack>>,
    /// Engine-owned imgui texture resolver, installed alongside the UI
    /// frame renderer (see `radiance_scripting::install_imgui_ui_renderer`).
    /// Shared with the renderer so immediate-mode composition
    /// (`with_ui_host`) and the retained layer stack resolve textures
    /// through the same cache. `None` until a renderer is installed.
    texture_resolver: RefCell<Option<Rc<RefCell<dyn TextureResolver>>>>,
}

impl UiManager {
    pub fn new(platform: &mut Platform) -> Self {
        let dpi_scale = platform.dpi_scale();
        Self {
            imgui_context: Rc::new(ImguiContext::new(platform)),
            ui: RefCell::new(None),
            dpi_scale,
            layers: Rc::new(RefCell::new(UiLayerStack::default())),
            texture_resolver: RefCell::new(None),
        }
    }

    pub fn imgui_context(&self) -> Rc<ImguiContext> {
        self.imgui_context.clone()
    }

    /// Register a game-shipped font (raw TTF/TTC bytes) for in-game text.
    /// Appended as an extra atlas slot; the bundled editor/title font is
    /// left in place. `extra_scale` is a per-game size multiplier applied on
    /// top of the per-face normalization. See [`ImguiContext::add_game_font`].
    pub fn add_game_font(&self, data: &[u8], extra_scale: f32) {
        self.imgui_context.add_game_font(data, extra_scale);
    }

    /// `FontId` of the registered game font for the given
    /// [`crate::imgui::GameFontSize`] slot, or `None` if none registered.
    pub fn game_font(&self, size: usize) -> Option<imgui::FontId> {
        self.imgui_context.game_font(size)
    }

    pub fn update(&self, delta_sec: f32, draw_func: impl Fn(&Ui)) -> ImguiFrame {
        let context = self.imgui_context.clone();
        let frame = context.draw_ui(delta_sec, |ui| {
            // Leak it. This is safe because we're only using it for the duration of this function.
            self.ui.replace(unsafe { Some(&*(ui as *const Ui)) });
            draw_func(ui);
            self.ui.replace(None);
        });

        frame
    }

    pub fn ui(&self) -> &'static Ui {
        self.ui
            .borrow()
            .expect("UI is not available outside of the update function")
    }

    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    /// Register a UI layer under `band`. The layer's `render` is driven
    /// once per frame inside the engine's imgui scope, in band order
    /// (and, within a band, registration order). Dropping the returned
    /// [`UiLayerHandle`] unregisters it.
    pub fn register_ui_layer(&self, band: UiLayerBand, layer: ComRc<IUiLayer>) -> UiLayerHandle {
        ui_layer::register(&self.layers, band, layer)
    }

    /// Snapshot of the registered UI layers in render order. Consumed by
    /// the engine each frame to drive `render`.
    pub fn ui_layers(&self) -> Vec<ComRc<IUiLayer>> {
        ui_layer::ordered(&self.layers)
    }

    /// Install the engine-owned imgui texture resolver. Called once when
    /// the UI frame renderer is installed; the same cache backs both the
    /// retained layer stack and immediate-mode `with_ui_host` composition.
    pub fn set_texture_resolver(&self, resolver: Rc<RefCell<dyn TextureResolver>>) {
        *self.texture_resolver.borrow_mut() = Some(resolver);
    }

    /// The engine-owned imgui texture resolver, or `None` if no UI frame
    /// renderer has been installed yet. Used by immediate-mode UI
    /// composition (`radiance_scripting`'s `with_ui_host` on `UiManager`).
    pub fn texture_resolver(&self) -> Option<Rc<RefCell<dyn TextureResolver>>> {
        self.texture_resolver.borrow().clone()
    }
}
