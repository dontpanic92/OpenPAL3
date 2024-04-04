use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    comdef::{
        IAnimationEventObserver, IAnimationEventObserverImpl, IArmatureComponent, IComponentImpl,
        IEntity, IScene,
    },
    components::mesh::{
        event::AnimationEvent,
        skinned_mesh::{AnimKeyFrame, AnimationState},
    },
    input::InputEngine,
    math::{Mat44, Vec3},
};

use crate::{
    utils::{get_camera_rotation, get_moving_direction},
    ComObject_Pal4ActorAnimationController, ComObject_Pal4ActorController,
};

use super::comdef::{
    IPal4ActorAnimationController, IPal4ActorAnimationControllerImpl, IPal4ActorControllerImpl,
};

pub struct Pal4ActorAnimationController {
    armature: ComRc<IArmatureComponent>,
    animation_config: RefCell<Pal4ActorAnimationConfig>,
}

ComObject_Pal4ActorAnimationController!(super::Pal4ActorAnimationController);

impl Pal4ActorAnimationController {
    pub fn create(armature: ComRc<IArmatureComponent>) -> ComRc<IPal4ActorAnimationController> {
        let controller = ComRc::<IPal4ActorAnimationController>::from_object(Self {
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

impl IPal4ActorAnimationControllerImpl for Pal4ActorAnimationController {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pal4ActorAnimationConfig {
    OneTime,
    Looping,
    PauseOnHold,
}

struct Pal4ActorControllerInner {
    input: Rc<RefCell<dyn InputEngine>>,
    entity: ComRc<IEntity>,
    scene: ComRc<IScene>,
    lock_control: bool,
    camera_rotation: f32,
}

impl Pal4ActorControllerInner {
    fn new(
        input: Rc<RefCell<dyn InputEngine>>,
        entity: ComRc<IEntity>,
        scene: ComRc<IScene>,
    ) -> Self {
        Self {
            input,
            entity,
            scene,
            lock_control: false,
            camera_rotation: 0.,
        }
    }

    fn update(&mut self, delta_sec: f32) {
        if self.lock_control {
            return;
        }

        let speed = 175.;
        let current_position = self.entity.transform().borrow().position();
        let direction = get_moving_direction(self.input.clone(), self.scene.clone());

        let target_position =
            Vec3::add(&current_position, &Vec3::dot(speed * delta_sec, &direction));

        if direction.norm() > 0.5 {
            let look_at = Vec3::new(target_position.x, current_position.y, target_position.z);
            self.entity
                .transform()
                .borrow_mut()
                .look_at(&look_at)
                .set_position(&target_position);
        }

        self.camera_rotation =
            get_camera_rotation(self.input.clone(), self.camera_rotation, delta_sec);
        self.scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&Vec3::new(400., 400., 400.))
            .rotate_axis_angle(&Vec3::UP, self.camera_rotation)
            .translate(&target_position)
            .look_at(&target_position);
    }
}

pub struct Pal4ActorController {
    inner: RefCell<Pal4ActorControllerInner>,
}

ComObject_Pal4ActorController!(super::Pal4ActorController);

impl Pal4ActorController {
    pub fn create(
        input: Rc<RefCell<dyn InputEngine>>,
        entity: ComRc<IEntity>,
        scene: ComRc<IScene>,
    ) -> Pal4ActorController {
        Self {
            inner: RefCell::new(Pal4ActorControllerInner::new(input, entity, scene)),
        }
    }
}

impl IComponentImpl for Pal4ActorController {
    fn on_loading(&self) {}

    fn on_updating(&self, delta_sec: f32) {
        self.inner.borrow_mut().update(delta_sec);
    }

    fn on_unloading(&self) {}
}

impl IPal4ActorControllerImpl for Pal4ActorController {
    fn lock_control(&self, lock: bool) -> () {
        self.inner.borrow_mut().lock_control = lock;
    }
}
