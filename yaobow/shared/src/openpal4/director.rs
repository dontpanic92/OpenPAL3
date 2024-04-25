use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirectorImpl, ISceneManager},
    input::InputEngine,
    radiance::{TaskManager, UiManager},
    rendering::ComponentFactory,
    scene::CoreScene,
    utils::free_view::FreeViewController,
};

use crate::{scripting::angelscript::ScriptVm, ComObject_OpenPAL4Director};

use super::{
    app_context::Pal4AppContext, asset_loader::AssetLoader, comdef::IOpenPAL4DirectorImpl,
    scripting::create_script_vm,
};

pub struct OpenPAL4Director {
    vm: RefCell<ScriptVm<Pal4AppContext>>,
    control: FreeViewController,
}

ComObject_OpenPAL4Director!(super::OpenPAL4Director);

impl OpenPAL4Director {
    pub fn new(
        component_factory: Rc<dyn ComponentFactory>,
        loader: Rc<AssetLoader>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
        input: Rc<RefCell<dyn InputEngine>>,
        audio: Rc<dyn AudioEngine>,
        task_manager: Rc<TaskManager>,
    ) -> Self {
        let app_context = Pal4AppContext::new(
            component_factory,
            loader,
            scene_manager,
            ui,
            input.clone(),
            audio,
            task_manager,
        );
        Self {
            vm: RefCell::new(create_script_vm(app_context)),
            control: FreeViewController::new(input),
        }
    }
}

impl IOpenPAL4DirectorImpl for OpenPAL4Director {
    fn get(&self) -> &'static crate::openpal4::director::OpenPAL4Director {
        unsafe { &*(self as *const _) }
    }
}

impl IDirectorImpl for OpenPAL4Director {
    fn activate(&self) {
        self.vm
            .borrow()
            .app_context
            .scene_manager
            .push_scene(CoreScene::create());
    }

    fn update(&self, delta_sec: f32) -> Option<crosscom::ComRc<radiance::comdef::IDirector>> {
        self.vm.borrow_mut().app_context_mut().update(delta_sec);

        if self.vm.borrow().context.is_none() {
            let function = self
                .vm
                .borrow_mut()
                .app_context_mut()
                .try_trigger_scene_events(delta_sec);
            if let Some(function) = function {
                let module = self.vm.borrow().app_context.scene.module.clone().unwrap();
                self.vm
                    .borrow_mut()
                    .set_function_by_name2(module, &function);
            }
        }

        self.vm.borrow_mut().execute(delta_sec);

        /*if !self.vm.borrow().app_context().player_locked {
            self.control
                .update(scene_manager.scene().unwrap(), delta_sec)
        }*/

        None
    }
}
