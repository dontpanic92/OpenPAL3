use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    radiance::UiManager,
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
        ui: Rc<UiManager>,
    ) -> Self {
        let context = Rc::new(RefCell::new(SWD5Context::new(
            asset_loader.clone(),
            audio_engine,
            input.clone(),
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
    fn activate(&self, _scene_manager: ComRc<ISceneManager>) {}

    fn update(
        &self,
        _scene_manager: ComRc<ISceneManager>,
        _ui: &imgui::Ui,
        delta_sec: f32,
    ) -> Option<ComRc<IDirector>> {
        self.context.borrow_mut().update(delta_sec);
        self.vm.execute().unwrap();
        None
    }
}

impl IOpenSWD5DirectorImpl for OpenSWD5Director {
    fn get(&self) -> &'static crate::openswd5::director::OpenSWD5Director {
        unsafe { &*(self as *const _) }
    }
}
