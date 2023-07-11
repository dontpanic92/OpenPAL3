use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    comdef::{IDirectorImpl, ISceneManager},
    radiance::UiManager,
};

use crate::{scripting::angelscript::ScriptVm, ComObject_OpenPAL4Director};

use super::{
    app_context::Pal4AppContext, asset_loader::AssetLoader, comdef::IOpenPAL4DirectorImpl,
    scripting::create_script_vm,
};

pub struct OpenPAL4Director {
    vm: RefCell<ScriptVm<Pal4AppContext>>,
}

ComObject_OpenPAL4Director!(super::OpenPAL4Director);

impl OpenPAL4Director {
    pub fn new(
        loader: Rc<AssetLoader>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
    ) -> Self {
        let app_context = Pal4AppContext::new(loader, scene_manager, ui);
        Self {
            vm: RefCell::new(create_script_vm(app_context)),
        }
    }
}

impl IOpenPAL4DirectorImpl for OpenPAL4Director {
    fn get(&self) -> &'static crate::openpal4::director::OpenPAL4Director {
        unsafe { &*(self as *const _) }
    }
}

impl IDirectorImpl for OpenPAL4Director {
    fn activate(&self, scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>) {}

    fn update(
        &self,
        _scene_manager: crosscom::ComRc<radiance::comdef::ISceneManager>,
        _ui: &imgui::Ui,
        _delta_sec: f32,
    ) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
        self.vm.borrow_mut().execute();

        None
    }
}
