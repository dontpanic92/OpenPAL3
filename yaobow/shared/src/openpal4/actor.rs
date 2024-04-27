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
    math::Vec3,
    utils::ray_casting::RayCaster,
};

use crate::{
    utils::{get_camera_rotation, get_moving_direction},
    ComObject_Pal4ActorAnimationController, ComObject_Pal4ActorController,
};

use super::{
    asset_loader::AssetLoader,
    comdef::{
        IPal4ActorAnimationController, IPal4ActorAnimationControllerImpl, IPal4ActorControllerImpl,
    },
    scene::SceneEventTrigger,
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

        self.current.replace(Pal4ActorAnimation::Unknown);
    }

    fn unhold(&self) {
        self.animation_config
            .replace(Pal4ActorAnimationConfig::OneTime);
        self.armature.play();
    }

    fn animation_completed(&self) -> bool {
        self.armature.animation_state() == AnimationState::Stopped
    }

    fn set_default(
        &self,
        keyframes: Vec<Vec<radiance::components::mesh::skinned_mesh::AnimKeyFrame>>,
        events: Vec<radiance::components::mesh::event::AnimationEvent>,
    ) -> crosscom::Void {
        self.default_keyframes.replace(keyframes);
        self.default_events.replace(events);
    }

    fn play_default(&self) {
        self.play_animation(
            self.default_keyframes.borrow().clone(),
            self.default_events.borrow().clone(),
            Pal4ActorAnimationConfig::Looping,
        );

        self.current.replace(Pal4ActorAnimation::Idle);
    }

    fn play(
        &self,
        animation: crate::openpal4::actor::Pal4ActorAnimation,
        config: crate::openpal4::actor::Pal4ActorAnimationConfig,
    ) {
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

    fn current(&self) -> crate::openpal4::actor::Pal4ActorAnimation {
        self.current.borrow().clone()
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
    ray_caster: RayCaster,
    event_triggers: Vec<Rc<SceneEventTrigger>>,
    lock_control: bool,
    camera_rotation: f32,
}

impl Pal4ActorControllerInner {
    fn new(
        input: Rc<RefCell<dyn InputEngine>>,
        entity: ComRc<IEntity>,
        scene: ComRc<IScene>,
        ray_caster: RayCaster,
        event_triggers: Vec<Rc<SceneEventTrigger>>,
    ) -> Self {
        Self {
            input,
            entity,
            scene,
            ray_caster,
            event_triggers,
            lock_control: true,
            camera_rotation: 0.,
        }
    }

    fn update(&mut self, delta_sec: f32) {
        if self.lock_control {
            return;
        }

        const SPEED: f32 = 175.;
        const STEP_HEIGHT: f32 = 10.;
        const TRIGGER_HEIGHT: f32 = 10.;

        let current_position = self.entity.transform().borrow().position();
        let direction = get_moving_direction(self.input.clone(), self.scene.clone());

        let target_position = Vec3::add(
            &current_position,
            &Vec3::scalar_mul(SPEED * delta_sec, &direction),
        );

        let movement = Vec3::sub(&target_position, &current_position);
        for trigger in self.event_triggers.iter() {
            trigger.check(
                &Vec3::new(
                    current_position.x,
                    current_position.y + TRIGGER_HEIGHT,
                    current_position.z,
                ),
                &movement,
            );
        }

        let ray_origin = Vec3::new(
            target_position.x,
            target_position.y + STEP_HEIGHT,
            target_position.z,
        );
        let p = self.ray_caster.cast_aaray(
            &ray_origin,
            radiance::utils::ray_casting::AARayDirection::NY,
        );

        if let Some(p) = p {
            let animation_controller = self
                .entity
                .get_component(IPal4ActorAnimationController::uuid())
                .unwrap()
                .query_interface::<IPal4ActorAnimationController>()
                .unwrap();
            if direction.norm() > 0.5 {
                let target_position = Vec3::new(
                    target_position.x,
                    target_position.y + STEP_HEIGHT - p,
                    target_position.z,
                );
                let look_at = Vec3::new(current_position.x, target_position.y, current_position.z);
                self.entity
                    .transform()
                    .borrow_mut()
                    .set_position(&target_position)
                    .look_at(&look_at);

                if animation_controller.current() != Pal4ActorAnimation::Run {
                    animation_controller
                        .play(Pal4ActorAnimation::Run, Pal4ActorAnimationConfig::Looping);
                }
            } else {
                if animation_controller.current() != Pal4ActorAnimation::Idle {
                    animation_controller
                        .play(Pal4ActorAnimation::Idle, Pal4ActorAnimationConfig::Looping);
                }
            }
        }

        self.camera_rotation =
            get_camera_rotation(self.input.clone(), self.camera_rotation, delta_sec);
        self.scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&Vec3::new(300., 300., 300.))
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
        event_triggers: Vec<Rc<SceneEventTrigger>>,
        ray_caster: RayCaster,
    ) -> Pal4ActorController {
        Self {
            inner: RefCell::new(Pal4ActorControllerInner::new(
                input,
                entity,
                scene,
                ray_caster,
                event_triggers,
            )),
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
