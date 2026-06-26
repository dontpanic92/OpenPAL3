//! PAL5 story-runtime director — drives the Lua VM that runs PAL5's game
//! scripts. Construction mirrors `OpenSWD5Director` (build context + VM,
//! resume per frame), but the command handlers + Lua bridge are PAL5's
//! own (in [`super::context`] / [`super::commands`]).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IDirector, IDirectorImpl, ISceneExt, ISceneManager};
use radiance::input::{InputEngine, Key};
use radiance::math::Vec3;
use radiance::radiance::UiManager;
use radiance::utils::free_view::FreeViewController;

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

    /// Free-fly debug camera, toggled with the `` ` ``/tilde key. While
    /// active the director freezes the plot (the Lua VM and scripted
    /// visuals stop advancing) and drives the scene camera with the
    /// shared [`FreeViewController`], so the scene can be inspected
    /// without progressing the story. This is a director concern, not a
    /// script concern, so it lives here rather than in
    /// [`Pal5ScriptContext`].
    debug_cam: Cell<bool>,
    free_view: FreeViewController,
    input: Rc<RefCell<dyn InputEngine>>,
    scene_manager: ComRc<ISceneManager>,
    ui: Rc<UiManager>,
}

ComObject_Pal5StoryDirector!(super::Pal5StoryDirector);

impl Pal5StoryDirector {
    pub fn new(
        context: Pal5ScriptContext,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
    ) -> anyhow::Result<Self> {
        Self::with_agent_bridge(context, None, input, scene_manager, ui)
    }

    pub fn with_agent_bridge(
        context: Pal5ScriptContext,
        agent_bridge: Option<Rc<AgentBridge>>,
        input: Rc<RefCell<dyn InputEngine>>,
        scene_manager: ComRc<ISceneManager>,
        ui: Rc<UiManager>,
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

        let free_view = FreeViewController::new(input.clone());

        Ok(Self {
            vm,
            context,
            agent_bridge,
            debug_cam: Cell::new(false),
            free_view,
            input,
            scene_manager,
            ui,
        })
    }

    /// Clone the shared script-context handle. Used by the loader's
    /// PAL5 agent pump to build the per-command dispatch context.
    pub fn context(&self) -> Rc<RefCell<Pal5ScriptContext>> {
        self.context.clone()
    }

    /// Toggle the free-fly debug camera when the `` ` ``/tilde key is
    /// pressed. The shared [`FreeViewController`] drives the existing
    /// scene camera transform in place, so toggling on continues from
    /// wherever the scripted camera last was — no view jump.
    fn handle_debug_cam_toggle(&self) {
        if !self.input.borrow().get_key_state(Key::Tilde).pressed() {
            return;
        }
        let active = !self.debug_cam.get();
        self.debug_cam.set(active);
        if active {
            log::info!("PAL5 debug camera ON — WASD move, Q/E up/down, arrows look, ` to exit");
        } else {
            log::info!("PAL5 debug camera OFF — plot resumes");
        }
    }

    /// On-screen hint while the debug camera is active, plus the live camera
    /// eye position and look-at target so a bad-render viewpoint can be recorded
    /// and reproduced exactly.
    fn draw_debug_cam_overlay(&self) {
        // Read the live scene-camera transform: eye = position, look-at = eye +
        // forward (the camera's local -Z axis, matrix column 2 negated).
        let (eye, look) = self
            .scene_manager
            .scene()
            .map(|scene| {
                let camera = scene.camera();
                let t = camera.transform();
                let m = t.matrix();
                let p = t.position();
                let fwd = Vec3::new(-m[0][2], -m[1][2], -m[2][2]);
                let look = Vec3::add(&p, &Vec3::scalar_mul(100.0, &fwd));
                (p, look)
            })
            .unwrap_or((Vec3::new_zeros(), Vec3::new_zeros()));

        let ui = self.ui.ui();
        ui.window("pal5_debug_cam")
            .position([12.0, 12.0], imgui::Condition::Always)
            .always_auto_resize(true)
            .movable(false)
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .bg_alpha(0.6)
            .build(|| {
                ui.text_colored([0.4, 1.0, 0.4, 1.0], "DEBUG CAMERA (plot frozen)");
                ui.text("WASD move  Q/E up/down  arrows look  ` exit");
                ui.separator();
                ui.text(format!(
                    "eye    [{:.1}, {:.1}, {:.1}]",
                    eye.x, eye.y, eye.z
                ));
                ui.text(format!(
                    "target [{:.1}, {:.1}, {:.1}]",
                    look.x, look.y, look.z
                ));
            });
    }
    /// Whether the free-fly debug camera is currently active. Enabled by
    /// either the keyboard toggle (`` ` ``) or the agent server
    /// (`/v1/camera/debug` → `AgentBridge::debug_cam`).
    fn debug_cam_active(&self) -> bool {
        self.debug_cam.get()
            || self
                .agent_bridge
                .as_ref()
                .map_or(false, |b| b.debug_cam.get())
    }
}

impl IDirectorImpl for Pal5StoryDirector {
    fn activate(&self) {}

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        // Free-fly debug camera (toggled with `~`): when active, freeze
        // the plot entirely — skip the script context update and the VM
        // so neither the story nor the scripted camera advance — and
        // drive the scene camera manually instead.
        self.handle_debug_cam_toggle();
        if self.debug_cam_active() {
            if let Some(scene) = self.scene_manager.scene() {
                self.free_view.update(scene, delta_sec);
            }
            self.draw_debug_cam_overlay();
            return None;
        }

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
