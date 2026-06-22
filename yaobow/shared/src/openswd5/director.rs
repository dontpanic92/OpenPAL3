use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    input::InputEngine,
    radiance::UiManager,
    rendering::ComponentFactory,
    utils::free_view::FreeViewController,
};

use crate::agent_common::AgentBridge;
use crate::scripting::lua50_32::Lua5032Vm;

use super::{
    asset_loader::AssetLoader,
    scripting::{SWD5Context, create_lua_vm},
};

pub struct OpenSWD5Director {
    vm: Lua5032Vm<SWD5Context>,
    context: Rc<RefCell<SWD5Context>>,
    // Reserved for free-fly camera control wiring; kept to preserve the
    // controller's lifetime even though no path drives it yet.
    #[allow(dead_code)]
    control: FreeViewController,
    /// Agent-server bridge. `None` for a normal windowed launch;
    /// `Some(_)` when `--swd5 --agent-port` was passed, in which case
    /// `update` honours pause / fixed-step and fast-forward.
    agent_bridge: Option<Rc<AgentBridge>>,
}

impl OpenSWD5Director {
    pub fn new(
        asset_loader: Rc<AssetLoader>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_manager: ComRc<ISceneManager>,
        audio_engine: Rc<dyn AudioEngine>,
        component_factory: Rc<dyn ComponentFactory>,
        ui: Rc<UiManager>,
        agent_bridge: Option<Rc<AgentBridge>>,
    ) -> Self {
        let context = Rc::new(RefCell::new(SWD5Context::new(
            asset_loader.clone(),
            audio_engine,
            input.clone(),
            component_factory,
            scene_manager,
            ui,
        )));
        let vm = create_lua_vm(&asset_loader, context.clone()).unwrap();

        Self {
            vm,
            context,
            control: FreeViewController::new(input),
            agent_bridge,
        }
    }

    /// Clone the shared script-context handle. Used by
    /// `Swd5Service::pump_agent` to build the per-command dispatch
    /// context (snapshot reads).
    pub fn context(&self) -> Rc<RefCell<SWD5Context>> {
        self.context.clone()
    }
}

ComObject_OpenSWD5Director!(super::OpenSWD5Director);

impl IDirectorImpl for OpenSWD5Director {
    fn activate(&self) {}

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        // Pause / fixed-step gating: when an agent bridge is present
        // and paused, `advance` is false and `effective_dt` is 0, so
        // the script clock freezes until a `/v1/time/step` is queued.
        let (advance, effective_dt) = self
            .agent_bridge
            .as_ref()
            .map_or((true, delta_sec), |b| b.effective_dt(delta_sec));

        self.context.borrow_mut().update(effective_dt);

        // Fast-forward: collapse any pending sleep / message wait so
        // the VM resumes this frame.
        let fast_forward = self
            .agent_bridge
            .as_ref()
            .map_or(false, |b| b.fast_forward.get());
        if fast_forward {
            self.context.borrow_mut().fast_forward_skip();
        }

        if advance && !self.context.borrow().is_sleeping() {
            let sleep = self.vm.execute().unwrap();
            let sleep = if fast_forward { 0. } else { sleep * 0.1 };
            self.context.borrow_mut().sleep(sleep);
        }

        None
    }

    fn deactivate(&self) {}
}
