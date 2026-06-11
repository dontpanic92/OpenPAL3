//! `Pal4Session` — playthrough-lifetime game state.
//!
//! North-star phase 1: carve the durable / serializable subset of the
//! running PAL4 game out of [`Pal4AppContext`](super::app_context::Pal4AppContext)
//! into a dedicated, mode-agnostic container so it can later survive a
//! director (game-mode) switch — e.g. story → battle → story — without
//! being rebuilt.
//!
//! For now the session is still owned by `Pal4AppContext` (the story
//! director's `ScriptVm`), and `Pal4AppContext` exposes thin delegating
//! accessors so the scripting surface is untouched. Moving ownership up
//! to the app-lifetime `Pal4App` (so the session is *borrowed* by each
//! director rather than owned by one) is the explicit job of phase 2.
//!
//! Save / load orchestration lives here: the session knows how to fold a
//! [`RuntimeSnapshot`] of the live world into the durable
//! [`Pal4PersistentState`] and persist it, and how to read a slot back
//! into a `RuntimeSnapshot` the director applies to the running context.

use std::cell::RefCell;
use std::rc::Rc;

use radiance::math::{Transform, Vec3};

use super::states::persistent_state::{PAL4_APP_NAME, Pal4PersistentState};

/// Plain-data snapshot of the live runtime world captured at save time
/// (and produced at load time for the director to re-apply).
///
/// This decouples [`Pal4Session`]'s save / load from `Pal4AppContext`
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

/// One active PAL4 playthrough. Owns the serializable game progress and
/// the save / load policy for it.
pub struct Pal4Session {
    state: Pal4PersistentState,
}

impl Pal4Session {
    /// Fresh playthrough seeded with the PAL4 save namespace.
    pub fn new() -> Self {
        Self {
            state: Pal4PersistentState::new(PAL4_APP_NAME.to_string()),
        }
    }

    /// Wrap an already-loaded persistent state (e.g. from a save slot).
    pub fn from_state(state: Pal4PersistentState) -> Self {
        Self { state }
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

    /// Persist the current playthrough to `slot`. Scene / block /
    /// leader / player-locked already live in `state` (the session is
    /// their single source of truth — `Pal4AppContext` writes them here
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
/// director's `Pal4AppContext` holds a *clone* of this handle, so the
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
        // (written live by Pal4AppContext), so they are set directly;
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
}
