//! PAL5 story-runtime director — drives the Lua VM that runs PAL5's game
//! scripts. Construction mirrors `OpenSWD5Director` (build context + VM,
//! resume per frame), but the command handlers + Lua bridge are PAL5's
//! own (in [`super::context`] / [`super::commands`]).

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IDirector, IDirectorImpl};

use shared::scripting::lua50_32::Lua5032Vm;

use super::commands::create_lua_vm;
use super::context::Pal5ScriptContext;

/// Script id of the canonical "start a new game" entry (`NewGame.lua`).
const NEWGAME_SCRIPT_ID: u32 = 1;

pub struct Pal5StoryDirector {
    vm: Lua5032Vm<Pal5ScriptContext>,
    context: Rc<RefCell<Pal5ScriptContext>>,
}

ComObject_Pal5StoryDirector!(super::Pal5StoryDirector);

impl Pal5StoryDirector {
    pub fn new(context: Pal5ScriptContext) -> anyhow::Result<Self> {
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

        Ok(Self { vm, context })
    }
}

impl IDirectorImpl for Pal5StoryDirector {
    fn activate(&self) {}

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        // Per-frame visuals (camera lerp, fades, dialog, audio) always
        // advance, even while the script is sleeping or finished.
        self.context.borrow_mut().update(delta_sec);

        if self.context.borrow().is_finished() {
            return None;
        }

        if !self.context.borrow().is_sleeping() {
            match self.vm.execute() {
                Ok(sleep) => self.context.borrow_mut().set_sleep(sleep),
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
