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
        test();
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
            .set_position(&Vec3::new(5500.0, 612.1155, 2500.0));

        scene_manager
            .scene()
            .unwrap()
            .camera()
            .borrow_mut()
            .transform_mut()
            .look_at(&Vec3::new(4319.2227, 612.1155, 1708.5408));
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

fn test() {
    let content =
        std::fs::read("F:\\SteamLibrary\\steamapps\\common\\Chinese Paladin 5\\Map\\kuangfengzhai\\kuangfengzhai_0_0.mp")
            .unwrap();

    let content = content[0x3c..].to_vec();
    let data = miniz_oxide::inflate::decompress_to_vec_zlib(&content).unwrap();

    std::fs::write("f:\\test.bin", &data).unwrap();
}
