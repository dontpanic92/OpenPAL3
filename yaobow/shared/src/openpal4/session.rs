//! `Pal4Session` — playthrough-lifetime game state.
//!
//! North-star phase 1: carve the durable / serializable subset of the
//! running PAL4 game out of [`Pal4VmContext`](super::vm_context::Pal4VmContext)
//! into a dedicated, mode-agnostic container so it can later survive a
//! director (game-mode) switch — e.g. story → battle → story — without
//! being rebuilt.
//!
//! For now the session is still owned by `Pal4VmContext` (the story
//! director's `ScriptVm`), and `Pal4VmContext` exposes thin delegating
//! accessors so the scripting surface is untouched. Moving ownership up
//! to the app-lifetime `Pal4App` (so the session is *borrowed* by each
//! director rather than owned by one) is the explicit job of phase 2.
//!
//! Save / load orchestration lives here: the session knows how to fold a
//! [`RuntimeSnapshot`] of the live world into the durable
//! [`Pal4PersistentState`] and persist it, and how to read a slot back
//! into a `RuntimeSnapshot` the director applies to the running context.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use radiance::math::{Transform, Vec3};

use super::states::persistent_state::{PAL4_APP_NAME, Pal4PersistentState};

/// Plain-data snapshot of the live runtime world captured at save time
/// (and produced at load time for the director to re-apply).
///
/// This decouples [`Pal4Session`]'s save / load from `Pal4VmContext`
/// borrows: the director reads these scalars off the running context,
/// hands them to the session to persist, and on load applies the
/// restored values back (scene reload, leader reposition, camera, …).
#[derive(Debug, Clone, Default)]
pub struct RuntimeSnapshot {
    pub scene_name: String,
    pub block_name: String,
    pub leader: usize,
    pub position: Option<Vec3>,
    pub direction: Option<f32>,
    pub camera: Option<Transform>,
    pub player_locked: bool,
    pub script_globals: Vec<u32>,
}

/// Cross-frame coordination channels for an active playthrough.
///
/// These are *transient* (never persisted to disk) per-playthrough
/// flags / queues that the angelscript VM, the loading-overlay
/// driver in [`OpenPAL4Director`](super::director::OpenPAL4Director),
/// and the agent dispatcher in
/// [`Pal4Service`](super::service::Pal4Service) all need to read or
/// write. They used to live on `Pal4VmContext` (VM-owned) which
/// meant every agent-side write had to be threaded through the
/// active director just to reach the right `&mut`; moving them here
/// lets each subsystem touch the session directly.
///
/// All fields use interior mutability so accessors only need
/// `&Pal4Session`. Reset by [`Pal4Session::load_slot`] before a
/// snapshot is handed back so a stale queued world-map pick or
/// pending scene transition from the previous playthrough never
/// leaks into the loaded one.
#[derive(Default)]
pub struct Pal4SessionTransient {
    /// `true` while a `giShowWorldMap` continuation is waiting for
    /// a destination pick. Surfaced via `/v1/state.world_map_open`
    /// so external drivers know they must `POST /v1/world_map/choose`
    /// before the script can advance.
    world_map_open: Cell<bool>,
    /// Buffered world-map destination as `(scene, block)`. Set by
    /// `/v1/world_map/choose`, consumed by the `giShowWorldMap`
    /// continuation. `None` ≡ "no choice yet".
    world_map_choice: RefCell<Option<(String, String)>>,

    /// Items the active script has queued for the next
    /// `giShowSelectDialog` / `giShowCommonDialogInSelectMode`.
    /// Populated by `giSelectDialogAddItem`; cleared when the
    /// matching `*_get_last_select` returns the choice.
    pending_dialog_choices: RefCell<Vec<String>>,
    /// Choice index to return from the next `*_get_last_select`
    /// call. `None` ≡ "use the default (1)" — the legacy stubbed
    /// behaviour. Consumed (taken) on the next read.
    next_dialog_choice: Cell<Option<i32>>,

    /// Deferred scene-transition request, set by callers that want
    /// the loading overlay to cover the synchronous `load_scene`
    /// rather than blocking the game thread mid-frame.
    /// [`OpenPAL4Director::drive_loading_overlay`] drains this on
    /// LOAD_READY. `None` ≡ no transition in flight.
    pending_scene_load: RefCell<Option<(String, String)>>,
    /// `true` if the most recent [`request_scene_load`] was tagged
    /// silent (`giArenaLoad show_loading = 0` and similar). The
    /// transition director reads this when minting the in-game
    /// transition: silent transitions skip the painted
    /// PAINTING / HOLDING overlay frames and run the scene swap
    /// inline (still through the deferred-generation machinery so
    /// VM continuations remain consistent across both flows).
    pending_scene_load_silent: Cell<bool>,
    /// `true` once the most recent deferred load has been applied
    /// via `load_scene`. Read by the scripted continuations so they
    /// only resume after the new scene is fully loaded.
    last_deferred_load_succeeded: Cell<bool>,
    /// Generation counter incremented on each deferred load
    /// (success or failure). Continuations capture the generation
    /// when they yield and resume only once it has advanced —
    /// independent of the scene name so re-entering the same scene
    /// also unblocks.
    deferred_load_generation: Cell<u64>,
}

/// One active PAL4 playthrough. Owns the serializable game progress and
/// the save / load policy for it.
pub struct Pal4Session {
    state: Pal4PersistentState,
    transient: Pal4SessionTransient,
}

impl Pal4Session {
    /// Fresh playthrough seeded with the PAL4 save namespace.
    pub fn new() -> Self {
        Self {
            state: Pal4PersistentState::new(PAL4_APP_NAME.to_string()),
            transient: Pal4SessionTransient::default(),
        }
    }

    /// Wrap an already-loaded persistent state (e.g. from a save slot).
    pub fn from_state(state: Pal4PersistentState) -> Self {
        Self {
            state,
            transient: Pal4SessionTransient::default(),
        }
    }

    pub fn state(&self) -> &Pal4PersistentState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut Pal4PersistentState {
        &mut self.state
    }

    /// Overwrite the entire durable state (used when loading a slot via
    /// an external [`Pal4PersistentState`]).
    pub fn replace_state(&mut self, state: Pal4PersistentState) {
        self.state = state;
    }

    pub fn app_name(&self) -> &str {
        self.state.app_name()
    }

    // === Cross-frame coordination channels (transient) ===============
    //
    // All `&self` — interior mutability on Cell/RefCell. The agent
    // dispatcher, the loading-overlay driver, and the scripting
    // sysfn handlers all touch these through whatever clone of the
    // shared `Rc<RefCell<Pal4Session>>` they happen to hold.

    pub fn world_map_open(&self) -> bool {
        self.transient.world_map_open.get()
    }

    /// Mark the world map as open (called by `giShowWorldMap`'s
    /// continuation entry). Idempotent.
    pub fn open_world_map(&self) {
        self.transient.world_map_open.set(true);
    }

    /// Buffer a `(scene, block)` destination for the next world-map
    /// continuation tick. Wired to `/v1/world_map/choose`. Consumed
    /// on the next tick — agents must re-supply a choice for each
    /// `giShowWorldMap` prompt.
    pub fn buffer_world_map_choice(&self, scene: String, block: String) {
        *self.transient.world_map_choice.borrow_mut() = Some((scene, block));
    }

    /// Take the buffered world-map choice (if any) and mark the
    /// world map closed. Returning `None` ≡ "still waiting, keep
    /// looping". Called from the `giShowWorldMap` continuation.
    pub fn take_world_map_choice(&self) -> Option<(String, String)> {
        let choice = self.transient.world_map_choice.borrow_mut().take();
        if choice.is_some() {
            self.transient.world_map_open.set(false);
        }
        choice
    }

    /// Snapshot of items currently queued for the next select-dialog.
    /// Returns an owned `Vec` (rather than a `Ref`) so callers can
    /// drop the session borrow immediately — the list is short
    /// (a handful of strings) so the clone is cheap.
    pub fn dialog_choices(&self) -> Vec<String> {
        self.transient.pending_dialog_choices.borrow().clone()
    }

    /// Append one item to the pending dialog-choice list. Called
    /// from the `giSelectDialogAddItem` sysfn handler.
    pub fn push_dialog_choice(&self, item: String) {
        self.transient.pending_dialog_choices.borrow_mut().push(item);
    }

    /// Buffer a choice for the next `*_get_last_select` call.
    /// Wired to `/v1/dialog/choose`. The value is consumed on the
    /// next read (so each pick lasts for exactly one dialog).
    pub fn buffer_dialog_choice(&self, index: i32) {
        self.transient.next_dialog_choice.set(Some(index));
    }

    /// Take the buffered choice (or default to `1`) and clear the
    /// pending-items list. Called from the `*_get_last_select`
    /// sysfn handlers.
    pub fn take_dialog_choice(&self) -> i32 {
        let choice = self.transient.next_dialog_choice.take().unwrap_or(1);
        self.transient.pending_dialog_choices.borrow_mut().clear();
        choice
    }

    /// Arm a deferred scene transition. The director drains this on
    /// the next `update()` once the loading overlay has rendered,
    /// then calls [`take_pending_scene_load`] and runs `load_scene`
    /// synchronously while the overlay is still on screen. Callers
    /// (`giArenaLoad`, `giShowWorldMap`,
    /// `OpenPAL4Director::load_state`) `Yield` a continuation that
    /// resumes once [`deferred_load_generation`] advances. An
    /// overlapping request replaces the previous one — only the
    /// most recently requested transition runs.
    ///
    /// `silent` requests bypass the painted PAINTING / HOLDING
    /// overlay frames — the transition director still routes the
    /// scene swap through its Loading phase (so VM continuations
    /// see the generation bump and the scene cell update
    /// atomically), but no loading layout is rendered. Used by
    /// `giArenaLoad show_loading = 0`, where the original game
    /// intends an instant swap.
    pub fn request_scene_load(&self, scene_name: &str, block_name: &str, silent: bool) {
        *self.transient.pending_scene_load.borrow_mut() =
            Some((scene_name.to_string(), block_name.to_string()));
        self.transient.pending_scene_load_silent.set(silent);
    }

    pub fn has_pending_scene_load(&self) -> bool {
        self.transient.pending_scene_load.borrow().is_some()
    }

    /// `true` iff the pending scene load was tagged silent (no
    /// overlay frames). Read by [`super::transition::build_in_game_transition`]
    /// when constructing the transition director.
    pub fn pending_scene_load_silent(&self) -> bool {
        self.transient.pending_scene_load_silent.get()
    }

    /// Peek at the pending (scene, block) without draining it.
    /// Used by the director to arm the loading overlay's state
    /// machine with the same names the continuation will see; the
    /// actual drain still happens via [`take_pending_scene_load`]
    /// when the overlay flips to LOAD_READY.
    pub fn peek_pending_scene_load(&self) -> Option<(String, String)> {
        self.transient.pending_scene_load.borrow().clone()
    }

    pub fn take_pending_scene_load(&self) -> Option<(String, String)> {
        let drained = self.transient.pending_scene_load.borrow_mut().take();
        if drained.is_some() {
            self.transient.pending_scene_load_silent.set(false);
        }
        drained
    }

    /// Bump the generation + success flag after a deferred
    /// `load_scene` finishes. Continuations watching for completion
    /// resume once the generation has advanced past the value they
    /// captured when yielding.
    pub fn note_deferred_load_finished(&self, succeeded: bool) {
        self.transient
            .last_deferred_load_succeeded
            .set(succeeded);
        let prev = self.transient.deferred_load_generation.get();
        self.transient
            .deferred_load_generation
            .set(prev.wrapping_add(1));
    }

    pub fn deferred_load_generation(&self) -> u64 {
        self.transient.deferred_load_generation.get()
    }

    pub fn last_deferred_load_succeeded(&self) -> bool {
        self.transient.last_deferred_load_succeeded.get()
    }

    /// Reset all cross-frame coordination channels. Called by
    /// [`load_slot`](Self::load_slot) before returning the snapshot,
    /// so a stale queued world-map pick / pending load / dialog
    /// choice from the previous playthrough doesn't leak into the
    /// loaded one. The deferred-load generation counter is *not*
    /// reset (it's monotonically increasing across the process
    /// lifetime — resetting it could re-fire a yielded continuation
    /// that already captured a higher baseline).
    fn reset_transient(&mut self) {
        let prev_gen = self.transient.deferred_load_generation.get();
        self.transient = Pal4SessionTransient::default();
        self.transient.deferred_load_generation.set(prev_gen);
    }

    /// Persist the current playthrough to `slot`. Scene / block /
    /// leader / player-locked already live in `state` (the session is
    /// their single source of truth — `Pal4VmContext` writes them here
    /// directly), so save only needs to fold in the values that are
    /// *derived from the live scene* at save time — the leader's
    /// position + facing and the camera transform — plus the script
    /// globals snapshot. Negative slots are ignored by
    /// [`Pal4PersistentState::save`].
    pub fn save_runtime(
        &mut self,
        slot: i32,
        position: Option<Vec3>,
        direction: Option<f32>,
        camera: Option<Transform>,
        script_globals: Vec<u32>,
    ) {
        self.state.set_position(position);
        self.state.set_direction(direction);
        self.state.set_camera(camera);
        self.state.set_script_globals(script_globals);
        self.state.save(slot);
    }

    /// Load `slot` from disk into this session, returning the
    /// [`RuntimeSnapshot`] the director applies to the running context.
    /// Errors (propagated) when the slot file is missing or malformed —
    /// the caller decides whether that is fatal.
    ///
    /// Also clears the transient coordination channels (see
    /// [`reset_transient`](Self::reset_transient)).
    pub fn load_slot(&mut self, slot: i32) -> anyhow::Result<RuntimeSnapshot> {
        let state = Pal4PersistentState::load(self.app_name(), slot)?;
        let snapshot = RuntimeSnapshot {
            scene_name: state.scene_name().to_string(),
            block_name: state.block_name().to_string(),
            leader: state.leader(),
            position: state.position(),
            direction: state.direction(),
            camera: state.camera().cloned(),
            player_locked: state.player_locked(),
            script_globals: state.script_globals().to_vec(),
        };
        self.state = state;
        self.reset_transient();
        Ok(snapshot)
    }
}

impl Default for Pal4Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared, app-lifetime handle to the playthrough session.
///
/// Phase 7 (shared-ownership model): the session lives behind a single
/// `Rc<RefCell<Pal4Session>>` owned by `Pal4Service`. Each story
/// director's `Pal4VmContext` holds a *clone* of this handle, so the
/// session is the same object across mode switches and is reachable
/// from app-lifetime code (the agent dispatcher) — no move, no copy.
/// The previous `Pal4SessionPark` (which moved the session in/out) is
/// gone; `Pal4Service` owns the handle directly.
pub type Pal4SessionHandle = Rc<RefCell<Pal4Session>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_runtime_folds_scene_derived_values_into_state() {
        // Negative slot makes `Pal4PersistentState::save` a no-op, so
        // this exercises the runtime fold without touching disk.
        // scene/block/leader/player_locked are the session's own state
        // (written live by Pal4VmContext), so they are set directly;
        // position/direction/camera/globals are scene-derived and
        // folded in by `save_runtime`.
        let mut session = Pal4Session::new();
        session
            .state_mut()
            .set_scene("m05".to_string(), "3".to_string());
        session.state_mut().set_leader(2);
        session.state_mut().set_player_locked(true);

        session.save_runtime(
            -1,
            Some(Vec3::new(1.0, 2.0, 3.0)),
            Some(90.0),
            None,
            vec![7, 8, 9],
        );

        let state = session.state();
        assert_eq!(state.scene_name(), "m05");
        assert_eq!(state.block_name(), "3");
        assert_eq!(state.leader(), 2);
        let pos = state.position().expect("position set");
        assert_eq!((pos.x, pos.y, pos.z), (1.0, 2.0, 3.0));
        assert_eq!(state.direction(), Some(90.0));
        assert!(state.player_locked());
        assert_eq!(state.script_globals(), &[7, 8, 9]);
    }

    #[test]
    fn shared_handle_mutation_is_visible_through_other_clones() {
        // Two clones of the same handle observe each other's writes —
        // this is what lets the active director and app-lifetime code
        // (the agent dispatcher) share one live session.
        let handle: Pal4SessionHandle = Rc::new(RefCell::new(Pal4Session::new()));
        let director_view = handle.clone();
        let app_view = handle.clone();

        director_view.borrow_mut().state_mut().add_money(500);
        director_view
            .borrow_mut()
            .state_mut()
            .set_scene("m07".to_string(), "2".to_string());

        assert_eq!(app_view.borrow().state().money(), 500);
        assert_eq!(app_view.borrow().state().scene_name(), "m07");
        assert_eq!(app_view.borrow().state().block_name(), "2");
    }

    #[test]
    fn session_survives_story_battle_story_round_trip() {
        // Shared-ownership reproduction of the director hand-off: the
        // app-lifetime owner keeps the canonical handle; story A mutates
        // through its clone and is dropped (entering battle); the
        // returning story B gets a fresh clone of the *same* handle and
        // sees the progress.
        let owner: Pal4SessionHandle = Rc::new(RefCell::new(Pal4Session::new()));

        // Story A: clone the handle, accrue durable progress, drop it.
        {
            let story_a = owner.clone();
            story_a.borrow_mut().state_mut().add_money(500);
            story_a
                .borrow_mut()
                .state_mut()
                .set_scene("m07".to_string(), "2".to_string());
        } // story_a's clone dropped (battle); the owner keeps the session.

        // Story B: a fresh clone of the same handle.
        let story_b = owner.clone();
        assert_eq!(
            story_b.borrow().state().money(),
            500,
            "money must survive the story->battle->story round trip"
        );
        assert_eq!(story_b.borrow().state().scene_name(), "m07");
        assert_eq!(story_b.borrow().state().block_name(), "2");
    }

    #[test]
    #[should_panic(expected = "already borrowed")]
    fn nested_borrow_panics_documents_the_hazard() {
        // The cost of the shared model: a held shared borrow across a
        // mutable borrow panics at runtime (instead of failing to
        // compile). Accessors must therefore drop their guards at
        // statement end — this test documents the failure mode.
        let handle: Pal4SessionHandle = Rc::new(RefCell::new(Pal4Session::new()));
        let _held = handle.borrow();
        handle.borrow_mut().state_mut().add_money(1);
    }

    #[test]
    fn transient_channels_round_trip_with_interior_mutability() {
        // All coordination channels are `&self` so the agent
        // dispatcher / VM / overlay driver can write through any
        // clone of the shared session handle without acquiring a
        // mutable borrow. Verify each channel's set / read /
        // consume contract.
        let session = Pal4Session::new();

        // World-map open + choice.
        assert!(!session.world_map_open());
        session.open_world_map();
        assert!(session.world_map_open());
        assert_eq!(session.take_world_map_choice(), None);
        // No choice → take is a no-op on the open flag.
        assert!(session.world_map_open());
        session.buffer_world_map_choice("m01".to_string(), "2".to_string());
        let choice = session.take_world_map_choice();
        assert_eq!(choice, Some(("m01".to_string(), "2".to_string())));
        // Taking a choice clears the open flag.
        assert!(!session.world_map_open());

        // Dialog choices.
        session.push_dialog_choice("yes".to_string());
        session.push_dialog_choice("no".to_string());
        assert_eq!(session.dialog_choices(), vec!["yes".to_string(), "no".to_string()]);
        session.buffer_dialog_choice(7);
        assert_eq!(session.take_dialog_choice(), 7);
        // After take, choices are cleared and next call defaults to 1.
        assert!(session.dialog_choices().is_empty());
        assert_eq!(session.take_dialog_choice(), 1);

        // Deferred scene load.
        let gen0 = session.deferred_load_generation();
        assert!(!session.has_pending_scene_load());
        session.request_scene_load("Q01", "N01", false);
        assert!(session.has_pending_scene_load());
        assert_eq!(
            session.peek_pending_scene_load(),
            Some(("Q01".to_string(), "N01".to_string()))
        );
        let drained = session.take_pending_scene_load();
        assert_eq!(drained, Some(("Q01".to_string(), "N01".to_string())));
        assert!(!session.has_pending_scene_load());
        session.note_deferred_load_finished(true);
        assert!(session.last_deferred_load_succeeded());
        assert_eq!(session.deferred_load_generation(), gen0.wrapping_add(1));
    }

    #[test]
    fn load_slot_clears_transient_but_preserves_generation() {
        // load_slot must drop stale queued coordination so a queued
        // world-map pick / pending scene transition from before the
        // load doesn't fire against the freshly restored state.
        // The deferred-load *generation* counter, however, is
        // monotonic for the process: resetting it could re-fire a
        // continuation that captured a higher baseline.
        //
        // The slot doesn't actually exist on disk; we expect
        // load_slot to error and we then verify reset_transient
        // separately (via a fresh test) — but the success path is
        // exercised by the live PAL4 smoke. Here we test the
        // private reset_transient directly.
        let mut session = Pal4Session::new();
        session.open_world_map();
        session.buffer_dialog_choice(3);
        session.request_scene_load("m07", "1", false);
        session.note_deferred_load_finished(true);
        let gen_before = session.deferred_load_generation();

        session.reset_transient();

        assert!(!session.world_map_open());
        assert_eq!(session.take_world_map_choice(), None);
        assert!(!session.has_pending_scene_load());
        assert!(!session.last_deferred_load_succeeded());
        // Generation preserved across reset.
        assert_eq!(session.deferred_load_generation(), gen_before);
        // Default-1 fallback still applies for dialog choice.
        assert_eq!(session.take_dialog_choice(), 1);
    }
}
