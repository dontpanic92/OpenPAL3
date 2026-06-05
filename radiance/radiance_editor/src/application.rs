use std::borrow::BorrowMut;
use std::cell::RefCell;

use crosscom::ComRc;
use radiance::{
    comdef::{IApplication, IApplicationExt, IComponentImpl, IDirector},
    scene::CoreScene,
};

/// Factory closure that builds the initial editor director once the
/// engine is ready. Invoked from `on_loading` (i.e. AFTER the
/// first-resumed engine bootstrap), so the closure can freely call
/// `app.engine()`.
pub type DirectorFactory = Box<dyn FnOnce(ComRc<IApplication>) -> ComRc<IDirector>>;

pub struct EditorApplicationLoader {
    app: ComRc<IApplication>,
    /// Director factory, consumed on first `on_loading`. Wrapped in
    /// `RefCell<Option<…>>` so the `IComponentImpl::on_loading(&self)`
    /// signature can `take()` the closure without `&mut self`.
    director_factory: RefCell<Option<DirectorFactory>>,
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

        let factory = self
            .director_factory
            .borrow_mut()
            .take()
            .expect("EditorApplicationLoader::on_loading fired without a director factory");
        let director = factory(self.app.clone());

        self.app
            .engine()
            .borrow()
            .scene_manager()
            .push_scene(CoreScene::create());
        self.app
            .engine()
            .borrow()
            .scene_manager()
            .set_director(director);
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, _delta_sec: f32) {}
}

impl EditorApplicationLoader {
    pub fn new(app: ComRc<IApplication>, director_factory: DirectorFactory) -> Self {
        Self {
            app,
            director_factory: RefCell::new(Some(director_factory)),
        }
    }
}
