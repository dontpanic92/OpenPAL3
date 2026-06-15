//! Host-implemented `IPal4GameContext`.
//!
//! Replaces the fat `IPal4ActorControllerContext`. Carries only the
//! truly PAL4-specific surface the scripted actor controller needs:
//! the engine-driven party-leader index plus per-scene event-trigger
//! dispatch. Every other concern (input, raycast, camera, leader
//! entity, animation) flows in through generic scriptable engine
//! protos at controller-construction time, not through this object.

use std::cell::Cell;

use crosscom::ComRc;
use radiance::comdef::IScene;
use radiance::components::collision::CollisionWorldComponent;
use radiance::math::Vec3;
use radiance::scene::ISceneExt;

use super::comdef::{IPal4GameContext, IPal4GameContextImpl};

pub struct Pal4GameContext {
    current_leader: Cell<usize>,
    /// The scene whose collision world owns the EVF segment triggers.
    /// `check_event_triggers` drives the world's evaluation; the result
    /// is read back via `Pal4Scene::test_event_triggers`.
    scene: ComRc<IScene>,
}

ComObject_Pal4GameContext!(super::Pal4GameContext);

impl Pal4GameContext {
    pub fn create(scene: ComRc<IScene>) -> ComRc<IPal4GameContext> {
        ComRc::from_object(Self {
            current_leader: Cell::new(0),
            scene,
        })
    }

    /// Engine-side setter for the active party leader. Called by
    /// `Pal4VmContext::set_leader`. Script reads via
    /// `IPal4GameContext::current_leader()`.
    pub fn set_current_leader(&self, leader: usize) {
        self.current_leader.set(leader);
    }
}

impl IPal4GameContextImpl for Pal4GameContext {
    fn current_leader(&self) -> std::os::raw::c_int {
        self.current_leader.get() as i32
    }

    fn check_event_triggers(&self, ox: f32, oy: f32, oz: f32, mx: f32, my: f32, mz: f32) {
        let origin = Vec3::new(ox, oy, oz);
        let end = Vec3::new(ox + mx, oy + my, oz + mz);
        self.scene
            .collision_world()
            .inner::<CollisionWorldComponent>()
            .evaluate_segment_triggers(&origin, &end);
    }
}
