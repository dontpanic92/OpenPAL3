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
    scene_manager: ComRc<ISceneManager>,
    control: FreeViewController,
}

impl OpenPAL5Director {
    pub fn new(input: Rc<RefCell<dyn InputEngine>>, scene_manager: ComRc<ISceneManager>) -> Self {
        Self {
            scene_manager,
            control: FreeViewController::new(input),
        }
    }
}

ComObject_OpenPAL5Director!(super::OpenPAL5Director);

impl IDirectorImpl for OpenPAL5Director {
    fn activate(&self) {
        self.scene_manager
            .scene()
            .unwrap()
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&Vec3::new(5500.0, 612.1155, 2500.0));

        self.scene_manager
            .scene()
            .unwrap()
            .camera()
            .borrow_mut()
            .transform_mut()
            .look_at(&Vec3::new(4319.2227, 612.1155, 1708.5408));
    }

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        self.control
            .update(self.scene_manager.scene().unwrap(), delta_sec);

        None
    }
}

impl IOpenPAL5DirectorImpl for OpenPAL5Director {
    fn get(&self) -> &'static crate::openpal5::director::OpenPAL5Director {
        unsafe { &*(self as *const _) }
    }
}
