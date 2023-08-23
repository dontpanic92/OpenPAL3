use std::cell::RefCell;

use crosscom::ComRc;
use radiance::{
    comdef::{
        IAnimationEventObserver, IAnimationEventObserverImpl, IArmatureComponent, IComponentImpl,
        IEntity,
    },
    components::mesh::{
        event::AnimationEvent,
        skinned_mesh::{AnimKeyFrame, AnimationState},
    },
};

use crate::ComObject_Pal4CharacterController;

use super::comdef::{IPal4CharacterController, IPal4CharacterControllerImpl};

pub struct Pal4CharacterController {
    armature: ComRc<IArmatureComponent>,
    animation_config: RefCell<Pal4ActorAnimationConfig>,
}

ComObject_Pal4CharacterController!(super::Pal4CharacterController);

impl Pal4CharacterController {
    pub fn create(armature: ComRc<IArmatureComponent>) -> ComRc<IPal4CharacterController> {
        let controller = ComRc::<IPal4CharacterController>::from_object(Self {
            armature: armature.clone(),
            animation_config: RefCell::new(Pal4ActorAnimationConfig::OneTime),
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

impl IPal4CharacterControllerImpl for Pal4CharacterController {
    fn play_animation(
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
    }

    fn unhold(&self) {
        self.animation_config
            .replace(Pal4ActorAnimationConfig::OneTime);
        self.armature.play();
    }

    fn animation_completed(&self) -> bool {
        self.armature.animation_state() == AnimationState::Stopped
    }
}

impl IAnimationEventObserverImpl for Pal4CharacterController {
    fn on_animation_event(&self, event_name: &str) {
        if *self.animation_config.borrow() == Pal4ActorAnimationConfig::PauseOnHold
            && event_name.to_lowercase().as_str() == "hold"
        {
            self.armature.pause();
        }
    }
}

impl IComponentImpl for Pal4CharacterController {
    fn on_loading(&self) {}

    fn on_updating(&self, _: f32) {}

    fn on_unloading(&self) {}
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pal4ActorAnimationConfig {
    OneTime,
    Looping,
    PauseOnHold,
}
