//! PAL5 story-runtime director — drives the Lua VM that runs PAL5's game
//! scripts. Construction mirrors `OpenSWD5Director` (build context + VM,
//! resume per frame), but the command handlers + Lua bridge are PAL5's
//! own (in [`super::context`] / [`super::commands`]).

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IDirector, IDirectorImpl};

use shared::agent_common::AgentBridge;
use shared::scripting::lua50_32::Lua5032Vm;

use super::commands::create_lua_vm;
use super::context::Pal5ScriptContext;

/// Script id of the canonical "start a new game" entry (`NewGame.lua`).
const NEWGAME_SCRIPT_ID: u32 = 1;

pub struct Pal5StoryDirector {
    vm: Lua5032Vm<Pal5ScriptContext>,
    context: Rc<RefCell<Pal5ScriptContext>>,
    /// Agent-server bridge. `None` for a normal windowed launch;
    /// `Some(_)` when `--pal5 --agent-port` was passed, in which case
    /// `update` honours pause / fixed-step and fast-forward.
    agent_bridge: Option<Rc<AgentBridge>>,
}

ComObject_Pal5StoryDirector!(super::Pal5StoryDirector);

impl Pal5StoryDirector {
    pub fn new(context: Pal5ScriptContext) -> anyhow::Result<Self> {
        Self::with_agent_bridge(context, None)
    }

    pub fn with_agent_bridge(
        context: Pal5ScriptContext,
        agent_bridge: Option<Rc<AgentBridge>>,
    ) -> anyhow::Result<Self> {
        let context = Rc::new(RefCell::new(context));
        let vm = create_lua_vm(context.clone())?;

        // Load the `NewGame` entry script, then enter via the harness'
        // `__pal5_main` wrapper (which calls `NewGame()` and flags
        // completion when it returns).
        let (_name, source) = {
            let c = context.borrow();
            c.script_index()
                .load_source(c.asset_loader().vfs(), NEWGAME_SCRIPT_ID)?
        };
        vm.load_chunk(&source, "NewGame")?;
        vm.set_entry("__pal5_main")?;

        Ok(Self {
            vm,
            context,
            agent_bridge,
        })
    }

    /// Clone the shared script-context handle. Used by the loader's
    /// PAL5 agent pump to build the per-command dispatch context.
    pub fn context(&self) -> Rc<RefCell<Pal5ScriptContext>> {
        self.context.clone()
    }
}

impl IDirectorImpl for Pal5StoryDirector {
    fn activate(&self) {}

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        // Pause / fixed-step gating: when an agent bridge is present
        // and paused, `advance` is false and `effective_dt` is 0, so
        // the script clock freezes until a `/v1/time/step` is queued.
        let (advance, effective_dt) = self
            .agent_bridge
            .as_ref()
            .map_or((true, delta_sec), |b| b.effective_dt(delta_sec));

        // Per-frame visuals (camera lerp, fades, dialog, audio) always
        // advance, even while the script is sleeping or finished.
        self.context.borrow_mut().update(effective_dt);

        if self.context.borrow().is_finished() {
            return None;
        }

        // Fast-forward: collapse any pending Wait / dialog so the VM
        // resumes this frame.
        let fast_forward = self
            .agent_bridge
            .as_ref()
            .map_or(false, |b| b.fast_forward.get());
        if fast_forward {
            self.context.borrow_mut().fast_forward_skip();
        }

        if advance && !self.context.borrow().is_sleeping() {
            match self.vm.execute() {
                Ok(sleep) => {
                    let sleep = if fast_forward { 0.0 } else { sleep };
                    self.context.borrow_mut().set_sleep(sleep);
                }
                Err(e) => {
                    log::error!("PAL5: script VM stopped: {}", e);
                    self.context.borrow_mut().mark_finished();
                }
            }
        }

        None
    }

    fn deactivate(&self) {}
}
