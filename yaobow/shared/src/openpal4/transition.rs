//! `Pal4TransitionDirector` — the single owner of every PAL4 scene
//! transition.
//!
//! Three flows route through here:
//!
//! - **Menu → New Game** (`enter_new_game`): no scene preload; just
//!   paints the loading layout while the story director's opening
//!   script wakes up and arms its first `giArenaLoad`. That arena
//!   load then triggers another in-game transition that covers the
//!   actual scene load.
//! - **Menu → Load Game** (`enter_load_game`): pre-drains the save
//!   snapshot, then on the `Loading` phase loads the saved scene and
//!   applies the snapshot fan-out (leader, position, direction,
//!   camera, player-lock state). The story director's idle VM is left
//!   idle: its `activate` notices the now-populated session scene
//!   name and skips the new-game opening kick.
//! - **In-game `giArenaLoad` / world-map jump**: the story director,
//!   on detecting `session.has_pending_scene_load()`, builds a
//!   transition with itself as the `next` director. The transition
//!   drains the request, runs `load_scene` while the overlay covers
//!   the freeze, and hands the *same* `ComRc<IDirector>` back so VM
//!   continuations watching the load-generation bump observe the
//!   completed load with no other state lost.
//!
//! The director is also designed to host (but does not yet
//! implement) story↔battle round-trips. The `next` slot can carry
//! the suspended story director through a battle, and a future
//! `EnterBattle { keep_world_scene }` variant can skip the unload
//! step so the world scene survives for instant re-entry.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crosscom::ComRc;
use radiance::comdef::{IDirector, IDirectorImpl, IImmediateDirectorImpl, IUiHost};

use crate::scripting::angelscript::ScriptVm;

use super::{
    comdef::IPal4LoadingOverlay,
    director::OpenPAL4Director,
    scene::Pal4Scene,
    session::RuntimeSnapshot,
    vm_context::Pal4VmContext,
};

/// Minimum on-screen duration after the load step completes. Avoids
/// the loading layout strobing in and out for cheap sub-100 ms loads.
/// Mirrors the constant the p7 overlay carried before the state
/// machine moved here.
const MIN_HOLD_SECS: f32 = 0.5;

/// If the engine never delivers a paint frame (e.g. a headless pump,
/// or an agent `/v1/time/step` burst running many `update`s per real
/// frame), fall through to `Loading` after this many accumulated
/// `dt`s. Keeps the transition from wedging when no frames are
/// actually being presented.
const PAINT_TIMEOUT_SECS: f32 = 0.1;

/// Per-frame cap applied to `dt` while we drain `MIN_HOLD_SECS`. The
/// synchronous `load_scene` makes the *next* real frame's `delta_sec`
/// carry the entire load duration (1–2 s); without this clamp,
/// `hold_remaining -= dt` would go immediately negative and the
/// overlay would dismiss after a single paint frame. 1 / 30 keeps the
/// hold honest at any frame rate up to 30 FPS.
const HOLD_DT_CAP: f32 = 1.0 / 30.0;

/// What the transition is supposed to do during its `Loading` phase.
pub enum Pal4TransitionAction {
    /// Menu → New Game. No scene preload — the story director's
    /// opening script (function 0) arms its own `giArenaLoad` on
    /// its first tick, which triggers a second transition that
    /// covers the actual load. We still pass through the overlay
    /// for the brief PAINTING + HOLDING window so the menu → story
    /// swap doesn't show a black frame.
    EnterStoryNew,

    /// Menu → Load Game. Drained into a `RuntimeSnapshot` up-front
    /// so the transition can call `load_scene(snapshot.scene,
    /// snapshot.block)` synchronously inside `Loading` and apply
    /// the post-load fan-out (leader / position / direction /
    /// camera / player-lock) on the same frame.
    EnterStoryFromSave { snapshot: RuntimeSnapshot, slot: i32 },

    /// In-game scripted transition (`giArenaLoad` / world map). The
    /// `(scene, block)` is read from `session.peek_pending_scene_load`
    /// when the transition is constructed; `Loading` calls
    /// `take_pending_scene_load` + `load_scene` + bumps the
    /// `deferred_load_generation` so VM continuations resume.
    ChangeScene { scene: String, block: String },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum TransitionPhase {
    /// Drawing the loading layout; waiting for ≥ 1 painted frame
    /// before kicking the synchronous load. Falls through after
    /// `PAINT_TIMEOUT_SECS` so a headless harness still progresses.
    Painting,
    /// Run the action this tick (synchronously). Transitions to
    /// `Holding` immediately after.
    Loading,
    /// Keep painting for at least `MIN_HOLD_SECS` after the load so
    /// the overlay doesn't strobe in and out.
    Holding,
    /// Hand off `next` on the next `update`. `update` returns
    /// `Some(next)` exactly once when in this phase.
    Done,
}

pub struct Pal4TransitionDirector {
    /// The loading layout renderer. Built per-launch by
    /// `IPal4ScriptFactory::make_pal4_loading_overlay`. Its
    /// `request`/`tick`/`notify_load_complete`/`is_active`/`cancel`
    /// methods drive the p7-side cosmetic state machine; this
    /// director owns the *transition* state machine and just asks
    /// the overlay to render.
    overlay: ComRc<IPal4LoadingOverlay>,

    /// Shared handle to the next director's `ScriptVm`. For boot
    /// flows this is the freshly-built story director's VM; for
    /// in-game transitions it is the suspended story director's
    /// VM (same `Rc`). Borrowed mutably during `Loading` to call
    /// `vm_context_mut().load_scene(...)`.
    vm: Rc<RefCell<ScriptVm<Pal4VmContext>>>,

    /// Director to install on `Done`. For menu → story this is the
    /// freshly-built story director; for in-game transitions it is
    /// the *same* story director that built this transition. Taken
    /// out of the `RefCell` exactly once when `update` returns it.
    next: RefCell<Option<ComRc<IDirector>>>,

    /// The work the `Loading` phase performs. Drained on transition
    /// to `Loading`.
    action: RefCell<Option<Pal4TransitionAction>>,

    phase: Cell<TransitionPhase>,
    /// Counted by `render_im` so `update` knows when at least one
    /// paint frame has gone out.
    frames_painted: Cell<u32>,
    /// Accumulated `dt` since `Painting` began; backs the
    /// `PAINT_TIMEOUT_SECS` fall-through.
    paint_elapsed: Cell<f32>,
    /// Remaining `MIN_HOLD_SECS` budget for the `Holding` phase.
    hold_remaining: Cell<f32>,

    /// `true` to skip the painted PAINTING / HOLDING frames and run
    /// the action inline on the very first `update`. Used for
    /// `giArenaLoad show_loading = 0`: the original game intends
    /// an instant swap, so the transition director still serves as
    /// the single owner of scene loads, but renders no overlay.
    silent: Cell<bool>,
}

ComObject_Pal4TransitionDirector!(super::Pal4TransitionDirector);

impl Pal4TransitionDirector {
    pub fn new(
        overlay: ComRc<IPal4LoadingOverlay>,
        vm: Rc<RefCell<ScriptVm<Pal4VmContext>>>,
        next: ComRc<IDirector>,
        action: Pal4TransitionAction,
    ) -> Self {
        Self {
            overlay,
            vm,
            next: RefCell::new(Some(next)),
            action: RefCell::new(Some(action)),
            phase: Cell::new(TransitionPhase::Painting),
            frames_painted: Cell::new(0),
            paint_elapsed: Cell::new(0.0),
            hold_remaining: Cell::new(MIN_HOLD_SECS),
            silent: Cell::new(false),
        }
    }

    /// Toggle silent (overlay-skipping) mode. Must be called before
    /// the transition is installed as the active director — once
    /// `update` runs the phase machine, the flag is consulted to
    /// short-circuit PAINTING / HOLDING.
    pub fn set_silent(&self, silent: bool) {
        self.silent.set(silent);
    }

    /// Execute the queued action. Called once when the transition
    /// moves from `Painting` to `Loading`. Errors are logged and the
    /// transition still advances to `Holding` so the overlay
    /// dismisses cleanly — the user already saw the loading layout,
    /// and the alternative (wedging) is worse.
    fn run_load(&self) {
        let action = match self.action.borrow_mut().take() {
            Some(action) => action,
            None => return,
        };

        match action {
            Pal4TransitionAction::EnterStoryNew => {
                // No-op load: the story director's opening script
                // arms its own `giArenaLoad` on its first tick and
                // that triggers a second transition. We only get
                // here for the brief PAINTING → HOLDING window that
                // hides the menu → story director swap.
            }

            Pal4TransitionAction::EnterStoryFromSave { snapshot, slot } => {
                self.vm
                    .borrow_mut()
                    .g
                    .borrow_mut()
                    .restore_globals(&snapshot.script_globals);

                if snapshot.scene_name.is_empty() {
                    log::info!(
                        "Pal4TransitionDirector: load slot {} has no scene to restore",
                        slot
                    );
                    return;
                }

                match self.swap_scene(&snapshot.scene_name, &snapshot.block_name) {
                    Ok(()) => {
                        let mut vm = self.vm.borrow_mut();
                        OpenPAL4Director::apply_snapshot(vm.vm_context_mut(), &snapshot);
                        log::info!(
                            "Game loaded from slot {} (via transition director)",
                            slot
                        );
                    }
                    Err(err) => {
                        log::error!(
                            "Pal4TransitionDirector::EnterStoryFromSave: load_scene \
                             scene='{}' block='{}' failed: {:?}; save state partially \
                             restored (globals applied, scene not changed)",
                            snapshot.scene_name,
                            snapshot.block_name,
                            err
                        );
                    }
                }
            }

            Pal4TransitionAction::ChangeScene { scene, block } => {
                // Drain the session-side request so a continuation
                // observing `peek_pending_scene_load` sees the load
                // has been consumed.
                let _ = self
                    .vm
                    .borrow()
                    .vm_context
                    .session()
                    .take_pending_scene_load();
                let succeeded = match self.swap_scene(&scene, &block) {
                    Ok(()) => true,
                    Err(err) => {
                        log::error!(
                            "Pal4TransitionDirector::ChangeScene: load_scene \
                             scene='{}' block='{}' failed: {:?}; aborting the \
                             surrounding script to keep the VM safe.",
                            scene,
                            block,
                            err
                        );
                        false
                    }
                };
                // Bump the deferred-load generation so VM
                // continuations (`giArenaLoad`, `giShowWorldMap`)
                // observing the previous value resume on the next
                // story tick.
                self.vm
                    .borrow()
                    .vm_context
                    .session()
                    .note_deferred_load_finished(succeeded);
            }
        }
    }

    /// Atomic scene swap: load a fresh `Pal4Scene`, repoint the
    /// engine `ISceneManager`, overwrite the shared scene cell, sync
    /// `session.state` to the new (scene, block), and re-apply the
    /// session's saved leader + player-lock to the freshly loaded
    /// scene. Replaces what was once `Pal4VmContext::load_scene` —
    /// the responsibility now lives at this director.
    fn swap_scene(&self, scene_name: &str, block_name: &str) -> anyhow::Result<()> {
        swap_pal4_scene(&self.vm, scene_name, block_name)
    }
}

/// Free-function scene swap shared by [`Pal4TransitionDirector`] and
/// [`OpenPAL4Director::load_state`] (the synchronous in-game F-key
/// loader). Loads a fresh `Pal4Scene` via the asset loader, pops +
/// pushes on the engine `ISceneManager`, overwrites the shared
/// `Rc<RefCell<Pal4Scene>>` cell that `Pal4VmContext` syscalls read
/// from, syncs `session.state` to the new (scene, block), and
/// re-applies the session's leader + player-lock to the freshly
/// loaded scene.
///
/// This used to live on `Pal4VmContext` as `load_scene`. Lifting it
/// out makes scene loading a responsibility of the *director* layer
/// (the transition director, or the story director's F-key
/// fallback) — `Pal4VmContext` only stores the cell.
pub(crate) fn swap_pal4_scene(
    vm: &Rc<RefCell<ScriptVm<Pal4VmContext>>>,
    scene_name: &str,
    block_name: &str,
) -> anyhow::Result<()> {
    // Collect the handles we need from the VM context. Each is
    // cheap to clone (Rc / ComRc / Rc<dyn>), so we don't hold a
    // borrow on `vm` across the synchronous load.
    let (loader, input, scene_manager, scene_cell, session, factory) = {
        let vm = vm.borrow();
        let app = vm.vm_context();
        (
            app.loader.clone(),
            app.input.clone(),
            app.scene_manager.clone(),
            app.scene.clone(),
            app.session_handle(),
            app.actor_controller_factory().cloned(),
        )
    };

    let _ = scene_manager.pop_scene();
    let scene = Pal4Scene::load(
        &loader,
        input,
        scene_name,
        block_name,
        factory.as_ref(),
    )?;
    let scene_root = scene.scene.clone();
    *scene_cell.borrow_mut() = scene;
    scene_manager.push_scene(scene_root);

    session
        .borrow_mut()
        .state_mut()
        .set_scene(scene_name.to_string(), block_name.to_string());

    // Re-apply the leader / lock state to the freshly loaded
    // scene (the previous scene's player entities are gone).
    let (leader, player_locked) = {
        let session = session.borrow();
        let state = session.state();
        (state.leader(), state.player_locked())
    };
    let mut vm = vm.borrow_mut();
    let app = vm.vm_context_mut();
    app.set_leader(leader as i32);
    app.lock_player(player_locked);
    Ok(())
}

impl IDirectorImpl for Pal4TransitionDirector {
    fn activate(&self) {
        // Arm the p7 overlay with the destination so its state
        // machine (used only for cosmetic logging today) sees the
        // same `(scene, block)` we will load. The destination is
        // best-effort: for `EnterStoryNew` we have none.
        let (scene, block) = match self.action.borrow().as_ref() {
            Some(Pal4TransitionAction::EnterStoryFromSave { snapshot, .. }) => (
                snapshot.scene_name.clone(),
                snapshot.block_name.clone(),
            ),
            Some(Pal4TransitionAction::ChangeScene { scene, block }) => {
                (scene.clone(), block.clone())
            }
            _ => (String::new(), String::new()),
        };
        self.overlay.request(&scene, &block);
        self.frames_painted.set(0);
        self.paint_elapsed.set(0.0);
        self.hold_remaining.set(MIN_HOLD_SECS);
        self.phase.set(TransitionPhase::Painting);
    }

    fn update(&self, delta_sec: f32) -> Option<ComRc<IDirector>> {
        // Silent transitions skip the painted PAINTING / HOLDING
        // frames: run the action immediately and hand off `next` on
        // the next `update` tick. This still gives us one frame of
        // overhead vs the legacy synchronous flow, but it preserves
        // the single invariant that scene loads happen exclusively
        // inside a transition director's Loading phase.
        if self.silent.get() {
            match self.phase.get() {
                TransitionPhase::Painting => {
                    self.phase.set(TransitionPhase::Loading);
                }
                TransitionPhase::Loading => {
                    self.run_load();
                    self.phase.set(TransitionPhase::Done);
                }
                TransitionPhase::Holding => {
                    self.phase.set(TransitionPhase::Done);
                }
                TransitionPhase::Done => {}
            }
            if self.phase.get() == TransitionPhase::Done {
                return self.next.borrow_mut().take();
            }
            return None;
        }

        match self.phase.get() {
            TransitionPhase::Painting => {
                self.paint_elapsed
                    .set(self.paint_elapsed.get() + delta_sec);
                if self.frames_painted.get() >= 1
                    || self.paint_elapsed.get() >= PAINT_TIMEOUT_SECS
                {
                    self.phase.set(TransitionPhase::Loading);
                }
                None
            }

            TransitionPhase::Loading => {
                self.run_load();
                self.overlay.notify_load_complete();
                self.hold_remaining.set(MIN_HOLD_SECS);
                self.phase.set(TransitionPhase::Holding);
                None
            }

            TransitionPhase::Holding => {
                // Clamp `dt`: the synchronous `load_scene` we just
                // ran inflated the next real frame's `delta_sec` to
                // 1–2 s. Subtracting that raw `dt` would drain
                // `MIN_HOLD_SECS` in a single tick, defeating the
                // entire point of the hold. Clamp to `HOLD_DT_CAP`
                // (1 / 30 s) so the on-screen dwell matches the
                // designed duration at any real frame rate up to
                // 30 FPS.
                let clamped = if delta_sec > HOLD_DT_CAP {
                    HOLD_DT_CAP
                } else {
                    delta_sec
                };
                let remaining = self.hold_remaining.get() - clamped;
                self.hold_remaining.set(remaining);
                if remaining <= 0.0 {
                    self.phase.set(TransitionPhase::Done);
                }
                None
            }

            TransitionPhase::Done => {
                // Dismiss the overlay layout before handing off so
                // the next director's render starts clean.
                self.overlay.cancel();
                self.next.borrow_mut().take()
            }
        }
    }

    fn deactivate(&self) {
        // Belt-and-braces: if we were torn down mid-transition
        // (e.g. a competing `scene_manager.set_director` from the
        // agent surface), make sure the overlay stops painting so
        // it doesn't leak into whatever director takes over.
        self.overlay.cancel();
    }
}

impl IImmediateDirectorImpl for Pal4TransitionDirector {
    fn render_im(&self, ui: ComRc<IUiHost>, dt: f32) {
        if self.phase.get() == TransitionPhase::Done {
            return;
        }
        self.overlay.render(ui, dt);
        // Saturate at 1000 so the counter doesn't wrap on a long
        // hold phase (~30 s at 30 FPS, well above MIN_HOLD_SECS).
        let painted = self.frames_painted.get();
        if painted < 1000 {
            self.frames_painted.set(painted + 1);
        }
    }
}

/// Build an in-game scene-transition director that wraps the story
/// director (passed as `&self` from `OpenPAL4Director::update`). The
/// returned `ComRc<IDirector>` is the transition; it carries a
/// `ComRc<IDirector>` clone of the story director in its `next` slot,
/// so on `Done` `scene_manager.set_director` resumes the same story
/// director with all its state (VM, continuations, debug bundle,
/// agent bridge) intact.
///
/// Falls back to a no-op transition (immediately re-installs the
/// story director) when no loading overlay is available (no script
/// factory installed — e.g. headless test harness). The caller will
/// then observe a one-frame swap which is functionally identical to
/// the pre-transition synchronous flow.
pub fn build_in_game_transition(story: &OpenPAL4Director) -> ComRc<IDirector> {
    let self_rc = ComRc::<IDirector>::from_self(story);

    let overlay = match story.loading_overlay_template() {
        Some(o) => o,
        None => {
            // No script factory → drain the request synchronously
            // and return ourselves so the scene_manager just
            // re-installs us (no real swap occurs because the
            // engine compares the new director and skips the no-op
            // case). This matches the legacy synchronous path.
            let vm = story.vm_handle();
            let pending = vm.borrow().vm_context.session().take_pending_scene_load();
            if let Some((scene, block)) = pending {
                let succeeded = swap_pal4_scene(&vm, &scene, &block).is_ok();
                vm.borrow()
                    .vm_context
                    .session()
                    .note_deferred_load_finished(succeeded);
            }
            return self_rc;
        }
    };

    let (pending, silent) = {
        let vm = story.vm_handle();
        let vm = vm.borrow();
        let session = vm.vm_context.session();
        (
            session.peek_pending_scene_load().unwrap_or_default(),
            session.pending_scene_load_silent(),
        )
    };
    let (scene, block) = pending;

    let transition = Pal4TransitionDirector::new(
        overlay,
        story.vm_handle(),
        self_rc,
        Pal4TransitionAction::ChangeScene { scene, block },
    );
    transition.set_silent(silent);
    ComRc::<IDirector>::from_object(transition)
}
