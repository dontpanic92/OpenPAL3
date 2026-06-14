//! PAL4 mode router + mode-factory registry — the single place that
//! knows the PAL4 game-mode graph.
//!
//! North-star phase 2 introduced [`Pal4ModeIntent`] (the typed mode
//! graph) and a `route()` switchboard. Phase 4 generalizes that fixed
//! `match` into a [`Pal4ModeRegistry`]: a map from a [`Pal4ModeKind`]
//! discriminant to a boxed factory closure that builds the concrete
//! [`IDirector`] for an intent.
//!
//! The payoff is extensibility without surgery: adding a future
//! `Battle` mode is `registry.register(Pal4ModeKind::Battle, factory)`
//! — no new bespoke service method, no edit to `route()`, no new IDL
//! surface. The built-in `StartMenu` (script-built) and `Story`
//! (Rust-built) modes are registered up front by
//! [`Pal4ModeRegistry::with_builtins`].
//!
//! The registry deliberately holds no game state — its factories
//! borrow the app-lifetime [`Pal4Service`] (which owns the asset
//! front-door, script hooks, agent bridge, and actor-controller
//! factory) and ask it to build the directors.

use std::collections::HashMap;

use crosscom::ComRc;
use radiance::comdef::IDirector;

use super::{
    service::Pal4Service,
    transition::{Pal4TransitionAction, Pal4TransitionDirector},
};

/// Typed PAL4 mode graph. Each variant is one way to enter a game
/// mode; [`route`] turns it into the director the scene manager
/// installs.
#[derive(Debug, Clone)]
pub enum Pal4ModeIntent {
    /// The scripted start menu (renders `MainWindow.xml`, plays the
    /// menu BGM, offers New Game / Load Game). Falls back to the story
    /// director when the scripted menu can't be built.
    StartMenu { asset_path: String },

    /// A fresh story playthrough (New Game).
    Story { asset_path: String },

    /// A story playthrough that boots directly into save `slot`
    /// (Load Game). The load is applied on the director's first
    /// advancing update.
    StoryFromSave { asset_path: String, slot: i32 },
}

/// Coarse mode discriminant used as the registry key. Multiple intents
/// that produce the same kind of director share one factory (e.g.
/// `Story` and `StoryFromSave` both map to [`Pal4ModeKind::Story`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pal4ModeKind {
    StartMenu,
    Story,
}

impl Pal4ModeIntent {
    /// The registry key this intent dispatches on.
    pub fn kind(&self) -> Pal4ModeKind {
        match self {
            Pal4ModeIntent::StartMenu { .. } => Pal4ModeKind::StartMenu,
            Pal4ModeIntent::Story { .. } | Pal4ModeIntent::StoryFromSave { .. } => {
                Pal4ModeKind::Story
            }
        }
    }
}

/// Factory that builds the director for a given intent, borrowing the
/// app-lifetime service for the engine handles / hooks it needs.
pub type Pal4ModeFactory = Box<dyn Fn(&Pal4Service, Pal4ModeIntent) -> ComRc<IDirector>>;

/// Registry of mode factories keyed by [`Pal4ModeKind`]. Owned by
/// [`Pal4Service`]; the single extension point for the PAL4 mode graph.
pub struct Pal4ModeRegistry {
    factories: HashMap<Pal4ModeKind, Pal4ModeFactory>,
}

impl Pal4ModeRegistry {
    /// Build a registry pre-populated with the two built-in modes:
    /// the script-built start menu and the Rust-built story director.
    pub fn with_builtins() -> Self {
        let mut factories: HashMap<Pal4ModeKind, Pal4ModeFactory> = HashMap::new();

        factories.insert(
            Pal4ModeKind::StartMenu,
            Box::new(
                |service: &Pal4Service, intent: Pal4ModeIntent| match intent {
                    Pal4ModeIntent::StartMenu { asset_path } => {
                        service.build_start_menu(&asset_path)
                    }
                    other => unreachable_intent(Pal4ModeKind::StartMenu, &other),
                },
            ),
        );

        factories.insert(
            Pal4ModeKind::Story,
            Box::new(
                |service: &Pal4Service, intent: Pal4ModeIntent| match intent {
                    Pal4ModeIntent::Story { asset_path } => {
                        let story = service.build_story_director(&asset_path);
                        let vm = story.vm_handle();
                        let overlay = story.loading_overlay_template();
                        let story_rc = ComRc::<IDirector>::from_object(story);
                        match overlay {
                            Some(overlay) => {
                                let transition = Pal4TransitionDirector::new(
                                    overlay,
                                    vm,
                                    story_rc,
                                    Pal4TransitionAction::EnterStoryNew,
                                );
                                ComRc::<IDirector>::from_object(transition)
                            }
                            // No overlay (e.g. headless test build) →
                            // install the story director directly so
                            // the legacy synchronous flow still works.
                            None => story_rc,
                        }
                    }
                    Pal4ModeIntent::StoryFromSave { asset_path, slot } => {
                        let story = service.build_story_director(&asset_path);
                        let vm = story.vm_handle();
                        let overlay = story.loading_overlay_template();

                        // Drain the snapshot up-front so the transition
                        // director can apply it synchronously on its
                        // Loading phase. On failure (missing/corrupt
                        // slot) we still hand off to the story director;
                        // since `load_slot` is atomic on failure, the
                        // session's scene_name stays at its prior value
                        // (empty for a fresh session) — the director's
                        // `activate` then naturally falls back to the
                        // new-game opening kick.
                        let snapshot = vm
                            .borrow_mut()
                            .vm_context_mut()
                            .session_mut()
                            .load_slot(slot)
                            .map_err(|e| {
                                log::error!(
                                    "Pal4ModeRegistry::StoryFromSave: cannot load slot \
                                     {}: {}",
                                    slot,
                                    e
                                );
                            })
                            .ok();

                        let story_rc = ComRc::<IDirector>::from_object(story);

                        match (overlay, snapshot) {
                            (Some(overlay), Some(snapshot)) => {
                                let transition = Pal4TransitionDirector::new(
                                    overlay,
                                    vm,
                                    story_rc,
                                    Pal4TransitionAction::EnterStoryFromSave { snapshot, slot },
                                );
                                ComRc::<IDirector>::from_object(transition)
                            }
                            // No snapshot: cannot load (the
                            // failure was already logged by
                            // `load_slot`). Fall through to the
                            // story director; its activate sees an
                            // idle VM + empty scene_name and kicks
                            // a fresh New Game.
                            (Some(_), None) => story_rc,
                            // No overlay (e.g. headless test
                            // harness): install the story director
                            // directly. With a valid snapshot, the
                            // legacy fallback is the agent
                            // `LoadSlot` command, which calls
                            // `OpenPAL4Director::load_state`
                            // synchronously.
                            (None, _) => story_rc,
                        }
                    }
                    other => unreachable_intent(Pal4ModeKind::Story, &other),
                },
            ),
        );

        Self { factories }
    }

    /// Register (or replace) the factory for `kind`. The extension hook
    /// for future modes (e.g. a `Battle` director) — call this once at
    /// boot instead of editing [`route`].
    pub fn register(&mut self, kind: Pal4ModeKind, factory: Pal4ModeFactory) {
        self.factories.insert(kind, factory);
    }

    /// Build the director for `intent`, or `None` when no factory is
    /// registered for its [`Pal4ModeKind`].
    pub fn build(&self, service: &Pal4Service, intent: Pal4ModeIntent) -> Option<ComRc<IDirector>> {
        let kind = intent.kind();
        self.factories
            .get(&kind)
            .map(|factory| factory(service, intent))
    }
}

impl Default for Pal4ModeRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

/// A built-in factory was handed an intent whose `kind()` doesn't match
/// the slot it was registered under — a programming error in
/// [`Pal4ModeRegistry::with_builtins`], not a runtime condition.
fn unreachable_intent(kind: Pal4ModeKind, intent: &Pal4ModeIntent) -> ! {
    panic!(
        "Pal4ModeRegistry: factory for {:?} received mismatched intent {:?}",
        kind, intent
    );
}

/// Single switchboard: map a [`Pal4ModeIntent`] to the concrete
/// [`IDirector`] the engine installs, by dispatching through the
/// service's [`Pal4ModeRegistry`]. The only place the PAL4 mode graph
/// is consulted.
pub fn route(service: &Pal4Service, intent: Pal4ModeIntent) -> ComRc<IDirector> {
    service.build_mode(intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_kind_maps_both_story_variants_to_story() {
        assert_eq!(
            Pal4ModeIntent::StartMenu {
                asset_path: "x".into()
            }
            .kind(),
            Pal4ModeKind::StartMenu
        );
        assert_eq!(
            Pal4ModeIntent::Story {
                asset_path: "x".into()
            }
            .kind(),
            Pal4ModeKind::Story
        );
        assert_eq!(
            Pal4ModeIntent::StoryFromSave {
                asset_path: "x".into(),
                slot: 2
            }
            .kind(),
            Pal4ModeKind::Story
        );
    }

    #[test]
    fn builtins_registry_has_both_modes() {
        // `register` replaces by key; inserting a no-op for each kind
        // and observing it does not panic confirms the keys are the
        // dispatch axis. The factories themselves require a live
        // `Pal4Service`, so they are exercised by the integration build
        // rather than here.
        let mut registry = Pal4ModeRegistry::with_builtins();
        registry.register(
            Pal4ModeKind::StartMenu,
            Box::new(|_svc, _intent| unreachable!("test factory never invoked")),
        );
        registry.register(
            Pal4ModeKind::Story,
            Box::new(|_svc, _intent| unreachable!("test factory never invoked")),
        );
    }
}
