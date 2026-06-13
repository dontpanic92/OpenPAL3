use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    comdef::{
        IAnimationEventObserver, IAnimationEventObserverImpl, IArmatureComponent,
        IArmatureComponentExt, IComponentImpl,
    },
    components::mesh::{
        event::AnimationEvent,
        skinned_mesh::{AnimKeyFrame, AnimationState},
    },
};

use super::{
    asset_loader::AssetLoader,
    comdef::{IPal4ActorAnimationController, IPal4ActorAnimationControllerImpl},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pal4ActorAnimation {
    Idle,
    Walk,
    Run,
    Unknown,
}

pub struct Pal4ActorAnimationController {
    actor_name: String,
    asset_loader: Rc<AssetLoader>,
    armature: ComRc<IArmatureComponent>,
    animation_config: RefCell<Pal4ActorAnimationConfig>,

    current: RefCell<Pal4ActorAnimation>,
    default_keyframes: RefCell<Vec<Vec<AnimKeyFrame>>>,
    default_events: RefCell<Vec<AnimationEvent>>,
}

ComObject_Pal4ActorAnimationController!(super::Pal4ActorAnimationController);

impl Pal4ActorAnimationController {
    pub fn create(
        actor_name: String,
        asset_loader: Rc<AssetLoader>,
        armature: ComRc<IArmatureComponent>,
    ) -> ComRc<IPal4ActorAnimationController> {
        let controller = ComRc::<IPal4ActorAnimationController>::from_object(Self {
            actor_name,
            asset_loader,
            armature: armature.clone(),
            animation_config: RefCell::new(Pal4ActorAnimationConfig::OneTime),
            current: RefCell::new(Pal4ActorAnimation::Unknown),
            default_keyframes: RefCell::new(Vec::new()),
            default_events: RefCell::new(Vec::new()),
        });

        armature.add_animation_event_observer(
            controller
                .clone()
                .query_interface::<IAnimationEventObserver>()
                .unwrap(),
        );

        controller
    }
}

impl IPal4ActorAnimationControllerImpl for Pal4ActorAnimationController {
    fn unhold(&self) {
        // Release a `PauseOnHold` action so it plays through its
        // remaining keyframes and naturally stops. PAL4's retail
        // semantics for the post-hold tail are *one-shot* — when
        // the script calls `giPlayerUnHoldAct` (or implicitly via
        // `giPlayerEndAction`) it expects the actor to finish the
        // action and return to whatever the default is, NOT to
        // start looping the tail. Force `armature.set_looping(false)`
        // alongside the state flip so an inadvertent earlier
        // `set_looping(true)` (e.g. a default-idle taken on the
        // shared armature) doesn't keep the controller in a
        // perpetual reset → reset → ... cycle. See M01/func2004
        // wedge investigation.
        self.animation_config
            .replace(Pal4ActorAnimationConfig::OneTime);
        self.armature.set_looping(false);
        self.armature.play();
    }

    fn animation_completed(&self) -> bool {
        // A Looping armature has no natural completion point — its
        // tick wraps back to 0 every `length` seconds. Reporting
        // "not completed" would wedge the script forever. Treat
        // any looping (or already-finished) state as "done";
        // playback continues visually but `giPlayerEndAction`
        // returns immediately. See actor.rs:unhold for the matching
        // unhold-side guard and the M01 wedge notes.
        if self
            .armature
            .inner::<radiance::components::mesh::skinned_mesh::ArmatureComponent>()
            .animation_looping()
        {
            return true;
        }
        matches!(
            self.armature.animation_state(),
            AnimationState::Stopped | AnimationState::NoAnimation
        )
    }

    fn play_default(&self) {
        self.play_animation(
            self.default_keyframes.borrow().clone(),
            self.default_events.borrow().clone(),
            Pal4ActorAnimationConfig::Looping,
        );

        self.current.replace(Pal4ActorAnimation::Idle);
    }

    fn play_by_id(&self, anim: i32, config: i32) {
        self.play(animation_from_i32(anim), animation_config_from_i32(config));
    }

    fn current_id(&self) -> i32 {
        animation_to_i32(self.current())
    }
}

impl Pal4ActorAnimationController {
    /// Inherent counterpart to the formerly-IDL `play_animation`.
    pub fn play_animation(
        &self,
        keyframes: Vec<Vec<AnimKeyFrame>>,
        events: Vec<AnimationEvent>,
        config: Pal4ActorAnimationConfig,
    ) {
        self.animation_config.replace(config);
        self.armature.set_animation(keyframes, events);

        match config {
            Pal4ActorAnimationConfig::OneTime | Pal4ActorAnimationConfig::PauseOnHold => {
                self.armature.set_looping(false)
            }
            Pal4ActorAnimationConfig::Looping => self.armature.set_looping(true),
        }

        self.current.replace(Pal4ActorAnimation::Unknown);
    }

    /// Inherent counterpart to the formerly-IDL `set_default`.
    pub fn set_default(&self, keyframes: Vec<Vec<AnimKeyFrame>>, events: Vec<AnimationEvent>) {
        self.default_keyframes.replace(keyframes);
        self.default_events.replace(events);
    }

    /// Inherent counterpart to the formerly-IDL `play`.
    pub fn play(&self, animation: Pal4ActorAnimation, config: Pal4ActorAnimationConfig) {
        let anim = match animation {
            Pal4ActorAnimation::Walk => self.asset_loader.load_animation(&self.actor_name, "C02"),
            Pal4ActorAnimation::Run => self.asset_loader.load_run_animation(&self.actor_name),
            _ => {
                self.play_default();
                return;
            }
        };

        if let Ok(anim) = anim {
            self.play_animation(anim.keyframes, anim.events, config);
            self.current.replace(animation);
        } else {
            log::error!(
                "Failed to load animation: {:?} for {}",
                animation,
                self.actor_name
            );
            self.play_default();
        }
    }

    /// Inherent counterpart to the formerly-IDL `current`.
    pub fn current(&self) -> Pal4ActorAnimation {
        self.current.borrow().clone()
    }

    /// Play a custom action by name (e.g. "C03"). Mirrors `play()` but
    /// loads the keyframes and AMF events for `act_name` instead of a
    /// hard-coded animation enum. Used by NPC scripts (`giNpcDoAction`
    /// et al.); the player side goes through
    /// `Pal4VmContext::player_do_action` which already does this
    /// inline via the player metadata.
    pub fn play_action(&self, act_name: &str, config: Pal4ActorAnimationConfig) {
        match self.asset_loader.load_animation(&self.actor_name, act_name) {
            Ok(anim) => {
                self.play_animation(anim.keyframes, anim.events, config);
                self.current.replace(Pal4ActorAnimation::Unknown);
            }
            Err(e) => {
                log::error!(
                    "Failed to load action '{}' for actor '{}': {:#}",
                    act_name,
                    self.actor_name,
                    e
                );
                self.play_default();
            }
        }
    }
}

/// Extension trait exposing `Pal4ActorAnimationController`'s formerly-IDL
/// accessors on a `ComRc<IPal4ActorAnimationController>` handle.
pub trait IPal4ActorAnimationControllerExt {
    fn set_default(&self, keyframes: Vec<Vec<AnimKeyFrame>>, events: Vec<AnimationEvent>);
    fn play(&self, animation: Pal4ActorAnimation, config: Pal4ActorAnimationConfig);
    fn play_action(&self, act_name: &str, config: Pal4ActorAnimationConfig);
    fn current(&self) -> Pal4ActorAnimation;
    fn play_animation(
        &self,
        keyframes: Vec<Vec<AnimKeyFrame>>,
        events: Vec<AnimationEvent>,
        config: Pal4ActorAnimationConfig,
    );
}

impl IPal4ActorAnimationControllerExt for ComRc<IPal4ActorAnimationController> {
    fn set_default(&self, keyframes: Vec<Vec<AnimKeyFrame>>, events: Vec<AnimationEvent>) {
        self.inner::<Pal4ActorAnimationController>()
            .set_default(keyframes, events)
    }
    fn play(&self, animation: Pal4ActorAnimation, config: Pal4ActorAnimationConfig) {
        self.inner::<Pal4ActorAnimationController>()
            .play(animation, config)
    }
    fn play_action(&self, act_name: &str, config: Pal4ActorAnimationConfig) {
        self.inner::<Pal4ActorAnimationController>()
            .play_action(act_name, config)
    }
    fn current(&self) -> Pal4ActorAnimation {
        self.inner::<Pal4ActorAnimationController>().current()
    }
    fn play_animation(
        &self,
        keyframes: Vec<Vec<AnimKeyFrame>>,
        events: Vec<AnimationEvent>,
        config: Pal4ActorAnimationConfig,
    ) {
        {
            let c = self.inner::<Pal4ActorAnimationController>();
            c.play_animation(keyframes, events, config)
        }
    }
}

impl IAnimationEventObserverImpl for Pal4ActorAnimationController {
    fn on_animation_event(&self, event_name: &str) {
        if *self.animation_config.borrow() == Pal4ActorAnimationConfig::PauseOnHold
            && event_name.to_lowercase().as_str() == "hold"
        {
            self.armature.pause();
        }
    }
}

impl IComponentImpl for Pal4ActorAnimationController {
    fn on_loading(&self) {}

    fn on_updating(&self, _: f32) {}

    fn on_unloading(&self) {}
}

pub(crate) fn animation_to_i32(a: Pal4ActorAnimation) -> i32 {
    match a {
        Pal4ActorAnimation::Idle => 0,
        Pal4ActorAnimation::Walk => 1,
        Pal4ActorAnimation::Run => 2,
        Pal4ActorAnimation::Unknown => 3,
    }
}

pub(crate) fn animation_from_i32(v: i32) -> Pal4ActorAnimation {
    match v {
        0 => Pal4ActorAnimation::Idle,
        1 => Pal4ActorAnimation::Walk,
        2 => Pal4ActorAnimation::Run,
        _ => Pal4ActorAnimation::Unknown,
    }
}

pub(crate) fn animation_config_from_i32(v: i32) -> Pal4ActorAnimationConfig {
    match v {
        0 => Pal4ActorAnimationConfig::OneTime,
        1 => Pal4ActorAnimationConfig::Looping,
        2 => Pal4ActorAnimationConfig::PauseOnHold,
        _ => Pal4ActorAnimationConfig::OneTime,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pal4ActorAnimationConfig {
    OneTime,
    Looping,
    PauseOnHold,
}
