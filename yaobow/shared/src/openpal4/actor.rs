use std::{cell::RefCell, rc::Rc};

use crosscom::ComRc;
use radiance::{
    comdef::{
        IAnimationEventObserver, IAnimationEventObserverImpl, IArmatureComponent,
        IArmatureComponentExt, IComponentImpl, IEntity, IEntityExt, IScene, ISceneExt,
    },
    components::mesh::{
        event::AnimationEvent,
        skinned_mesh::{AnimKeyFrame, AnimationState},
    },
    input::InputEngine,
    math::Vec3,
    utils::ray_casting::RayCaster,
};

use crate::utils::{get_camera_rotation, get_moving_direction};

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
    fn unhold(&self) {
        self.animation_config
            .replace(Pal4ActorAnimationConfig::OneTime);
        self.armature.play();
    }

    fn animation_completed(&self) -> bool {
        self.armature.animation_state() == AnimationState::Stopped
    }

    fn play_default(&self) {
        self.play_animation(
            self.default_keyframes.borrow().clone(),
            self.default_events.borrow().clone(),
            Pal4ActorAnimationConfig::Looping,
        );

        self.current.replace(Pal4ActorAnimation::Idle);
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
}

/// Extension trait exposing `Pal4ActorAnimationController`'s formerly-IDL
/// accessors on a `ComRc<IPal4ActorAnimationController>` handle.
pub trait IPal4ActorAnimationControllerExt {
    fn set_default(&self, keyframes: Vec<Vec<AnimKeyFrame>>, events: Vec<AnimationEvent>);
    fn play(&self, animation: Pal4ActorAnimation, config: Pal4ActorAnimationConfig);
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pal4ActorAnimationConfig {
    OneTime,
    Looping,
    PauseOnHold,
}

/// Per-scene collision/input state shared by every party member's
/// `Pal4ActorController` wrapper. There is exactly one of these per
/// loaded `Pal4Scene`; the per-entity wrappers (one on each of the
/// four player entities) hold a `Rc<RefCell<…>>` to this and only
/// the wrapper whose `player_id` matches `current_leader` actually
/// drives the update on its own entity each frame.
///
/// This indirection is what makes the floor/wall raycast follow the
/// active leader after `set_leader` switches party members or after
/// a scene reload — both the `ray_caster` and the `event_triggers`
/// are bound to the *current* scene, and the wrapper on the new
/// leader entity is the one that ticks.
pub struct Pal4ActorControllerInner {
    input: Rc<RefCell<dyn InputEngine>>,
    scene: ComRc<IScene>,
    ray_caster: RayCaster,
    event_triggers: Vec<Rc<SceneEventTrigger>>,
    locked: bool,
    camera_rotation: f32,
    camera_height: f32,
    current_leader: usize,
}

impl Pal4ActorControllerInner {
    pub(crate) fn new(
        input: Rc<RefCell<dyn InputEngine>>,
        scene: ComRc<IScene>,
        ray_caster: RayCaster,
        event_triggers: Vec<Rc<SceneEventTrigger>>,
    ) -> Self {
        Self {
            input,
            scene,
            ray_caster,
            event_triggers,
            locked: true,
            camera_rotation: 0.,
            camera_height: 300.,
            current_leader: 0,
        }
    }

    pub(crate) fn set_current_leader(&mut self, leader: usize) {
        self.current_leader = leader;
    }

    pub(crate) fn current_leader(&self) -> usize {
        self.current_leader
    }

    pub(crate) fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    fn update(&mut self, entity: ComRc<IEntity>, delta_sec: f32) {
        if self.locked {
            return;
        }

        const SPEED: f32 = 175.;
        const STEP_HEIGHT: f32 = 10.;
        const TRIGGER_HEIGHT: f32 = 10.;

        let current_position = entity.transform().borrow().position();
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
            let animation_controller = entity
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
                entity
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
        self.camera_height = get_camera_height(self.input.clone(), self.camera_height, delta_sec);
        {
            let mut c = self.scene.camera_mut();
            c.transform_mut()
                .set_position(&Vec3::new(300., self.camera_height, 300.))
                .rotate_axis_angle(&Vec3::UP, self.camera_rotation)
                .translate(&target_position)
                .look_at(&target_position);
        };
    }
}

fn get_camera_height(
    input: Rc<RefCell<dyn InputEngine>>,
    current_height: f32,
    delta_sec: f32,
) -> f32 {
    const SPEED: f32 = 100.;

    let mut height = current_height;
    if input
        .borrow()
        .get_key_state(radiance::input::Key::W)
        .is_down()
    {
        height += SPEED * delta_sec;
    }
    if input
        .borrow()
        .get_key_state(radiance::input::Key::S)
        .is_down()
    {
        height -= SPEED * delta_sec;
    }

    height
}

/// Per-entity wrapper attached to each of the four player entities
/// in a `Pal4Scene`. Each wrapper points at the shared
/// `Pal4ActorControllerInner` for the current scene; only the
/// wrapper whose `player_id` matches `inner.current_leader` actually
/// ticks. This guarantees that whichever party member is the active
/// leader receives the correct floor/wall collision against the
/// scene's freshly-built `RayCaster`.
pub struct Pal4ActorController {
    inner: Rc<RefCell<Pal4ActorControllerInner>>,
    entity: ComRc<IEntity>,
    player_id: usize,
}

ComObject_Pal4ActorController!(super::Pal4ActorController);

impl Pal4ActorController {
    pub fn create(
        inner: Rc<RefCell<Pal4ActorControllerInner>>,
        entity: ComRc<IEntity>,
        player_id: usize,
    ) -> Pal4ActorController {
        Self {
            inner,
            entity,
            player_id,
        }
    }
}

impl IComponentImpl for Pal4ActorController {
    fn on_loading(&self) {}

    fn on_updating(&self, delta_sec: f32) {
        let is_leader = self.inner.borrow().current_leader() == self.player_id;
        if is_leader {
            self.inner
                .borrow_mut()
                .update(self.entity.clone(), delta_sec);
        }
    }

    fn on_unloading(&self) {}
}

impl IPal4ActorControllerImpl for Pal4ActorController {
    fn lock_control(&self, lock: bool) -> () {
        self.inner.borrow_mut().set_locked(lock);
    }
}
