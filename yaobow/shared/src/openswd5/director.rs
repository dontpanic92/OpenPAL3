use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    utils::free_view::FreeViewController,
};

use crate::ComObject_OpenSWD5Director;

use super::comdef::IOpenSWD5DirectorImpl;

pub struct OpenSWD5Director {
    control: FreeViewController,
}

impl OpenSWD5Director {
    pub fn new(input: Rc<RefCell<dyn InputEngine>>) -> Self {
        Self {
            control: FreeViewController::new(input),
        }
    }
}

ComObject_OpenSWD5Director!(super::OpenSWD5Director);

impl IDirectorImpl for OpenSWD5Director {
    fn activate(&self, _scene_manager: ComRc<ISceneManager>) {}

    fn update(
        &self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &imgui::Ui,
        _delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        None
    }
}

impl IOpenSWD5DirectorImpl for OpenSWD5Director {
    fn get(&self) -> &'static crate::openswd5::director::OpenSWD5Director {
        unsafe { &*(self as *const _) }
    }
}
