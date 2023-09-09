use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    math::Vec3,
    utils::free_view::FreeViewController,
};

use crate::ComObject_OpenPAL5Director;

use super::comdef::IOpenPAL5DirectorImpl;

pub struct OpenPAL5Director {
    control: FreeViewController,
}

impl OpenPAL5Director {
    pub fn new(input: Rc<RefCell<dyn InputEngine>>) -> Self {
        Self {
            control: FreeViewController::new(input),
        }
    }
}

ComObject_OpenPAL5Director!(super::OpenPAL5Director);

impl IDirectorImpl for OpenPAL5Director {
    fn activate(&self, scene_manager: ComRc<ISceneManager>) {
        scene_manager
            .scene()
            .unwrap()
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&Vec3::new(1500.0, 1500.0, 1500.0));

        scene_manager
            .scene()
            .unwrap()
            .camera()
            .borrow_mut()
            .transform_mut()
            .look_at(&Vec3::new_zeros());
    }

    fn update(
        &self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {

        self.control
            .update(scene_manager.scene().unwrap(), delta_sec);

        None
    }
}

impl IOpenPAL5DirectorImpl for OpenPAL5Director {
    fn get(&self) -> &'static crate::openpal5::director::OpenPAL5Director {
        unsafe { &*(self as *const _) }
    }
}
