use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    math::{Quaternion, Vec3},
    radiance::UiManager,
    rendering::ComponentFactory,
    utils::free_view::FreeViewController,
};

use crate::{scripting::lua50_32::Lua5032Vm, ComObject_OpenSWD5Director};

use super::{
    asset_loader::AssetLoader,
    comdef::IOpenSWD5DirectorImpl,
    scripting::{create_lua_vm, SWD5Context},
};

pub struct OpenSWD5Director {
    vm: Lua5032Vm<SWD5Context>,
    context: Rc<RefCell<SWD5Context>>,
    control: FreeViewController,
}

impl OpenSWD5Director {
    pub fn new(
        asset_loader: Rc<AssetLoader>,
        input: Rc<RefCell<dyn InputEngine>>,
        audio_engine: Rc<dyn AudioEngine>,
        component_factory: Rc<dyn ComponentFactory>,
        ui: Rc<UiManager>,
    ) -> Self {
        let context = Rc::new(RefCell::new(SWD5Context::new(
            asset_loader.clone(),
            audio_engine,
            input.clone(),
            component_factory,
            ui,
        )));
        let vm = create_lua_vm(&asset_loader, context.clone()).unwrap();

        Self {
            vm,
            context,
            control: FreeViewController::new(input),
        }
    }
}

ComObject_OpenSWD5Director!(super::OpenSWD5Director);

impl IDirectorImpl for OpenSWD5Director {
    fn activate(&self, scene_manager: ComRc<ISceneManager>) {
        self.context.borrow_mut().set_scene_manager(scene_manager);
    }

    fn update(
        &self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        self.context.borrow_mut().update(delta_sec);
        self.vm.execute().unwrap();

        let scene = scene_manager.scene().unwrap();
        // self.control.update(scene, delta_sec);

        scene.camera().borrow_mut().set_fov43(60_f32.to_radians());

        scene
            .camera()
            .borrow_mut()
            .transform_mut()
            //.set_position(&Vec3::new(15.5, 35., 122.))
            //.look_at(&Vec3::new(-13., 7., -13.));
            .set_position(&Vec3::new(-13., 7., -13.))
            .look_at(&Vec3::new(-13., 7., -12.))
            // .rotate_quaternion_local(&Quaternion::new(-122_f32.to_radians(), -35_f32.to_radians(), 0., 0.))
            .rotate_axis_angle_local(&Vec3::UP, (-122.0_f32).to_radians())
            .rotate_axis_angle_local(&Vec3::EAST, (-35_f32).to_radians())
            .translate_local(&Vec3::new(0., 0., 15.5));

        None
    }
}

impl IOpenSWD5DirectorImpl for OpenSWD5Director {
    fn get(&self) -> &'static crate::openswd5::director::OpenSWD5Director {
        unsafe { &*(self as *const _) }
    }
}
