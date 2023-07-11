use std::borrow::BorrowMut;

use crosscom::ComRc;
use radiance::{
    comdef::{IApplication, IComponentImpl, IDirector},
    scene::CoreScene,
};

use crate::ComObject_EditorApplicationLoaderComponent;

pub struct EditorApplicationLoader {
    // plugin_create: Option<Box<dyn Fn(ComRc<IApplication>) -> SceneViewPlugins>>,
    app: ComRc<IApplication>,
    director: ComRc<IDirector>,
}

ComObject_EditorApplicationLoaderComponent!(super::EditorApplicationLoader);

impl IComponentImpl for EditorApplicationLoader {
    fn on_loading(&self) {
        self.app
            .engine()
            .borrow()
            .ui_manager()
            .imgui_context()
            .context_mut()
            .borrow_mut()
            .io_mut()
            .config_windows_move_from_title_bar_only = true;

        unsafe {
            (*imgui::sys::igGetIO()).FontDefault = std::slice::from_raw_parts_mut(
                (*(*imgui::sys::igGetIO()).Fonts).Fonts.Data,
                (*(*imgui::sys::igGetIO()).Fonts).Fonts.Size as usize,
            )[1]
        };

        self.app
            .engine()
            .borrow()
            .scene_manager()
            .push_scene(CoreScene::create());
        self.app
            .engine()
            .borrow()
            .scene_manager()
            .set_director(self.director.clone());
    }

    fn on_updating(&self, _delta_sec: f32) {}
}

impl EditorApplicationLoader {
    pub fn new(app: ComRc<IApplication>, director: ComRc<IDirector>) -> Self {
        Self { app, director }
    }
}
