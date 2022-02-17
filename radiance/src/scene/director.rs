use super::SceneManager;
use crate::ui::Ui;
use alloc::rc::Rc;
use core::cell::RefCell;

pub trait Director: downcast_rs::Downcast {
    fn activate(&mut self, scene_manager: &mut dyn SceneManager);
    fn update(
        &mut self,
        scene_manager: &mut dyn SceneManager,
        ui: &mut Ui,
        delta_sec: f32,
    ) -> Option<Rc<RefCell<dyn Director>>>;
}

downcast_rs::impl_downcast!(Director);
