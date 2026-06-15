//! [`CollisionWorldComponent`] — the scene-level aggregator for
//! colliders and trigger volumes.
//!
//! Hosted on the scene itself (obtained via
//! [`ISceneExt::collision_world`](crate::scene::ISceneExt::collision_world)),
//! it owns the bookkeeping games used to do by hand:
//!
//! * **Colliders** — `attach_collider` bakes an entity's static-mesh
//!   subtree into a single world-space [`RayCaster`]; the aggregate is
//!   queried by [`cast_ray`](CollisionWorldComponent::cast_ray) /
//!   [`cast_aa_ny`](CollisionWorldComponent::cast_aa_ny) and exposed to
//!   script as an [`IRayCaster`](crate::comdef::IRayCaster) via
//!   [`floor_ray_caster`](CollisionWorldComponent::floor_ray_caster).
//! * **Segment triggers** — `attach_segment_trigger` registers a
//!   world-space event region; `evaluate_segment_triggers` latches which
//!   ones a movement segment crossed this frame and
//!   `fired_segment_trigger` returns the first fired payload.
//! * **Proximity triggers** — `attach_proximity_trigger` registers an
//!   interaction probe; `nearest_proximity_trigger` returns the closest
//!   one within its own radius.
//!
//! Trigger payloads (`id` + `tag`) stay opaque — the engine never
//! interprets them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crosscom::ComRc;

use crate::comdef::{
    ICollisionWorldComponentImpl, IComponentImpl, IEntity, IEntityExt, IRayCaster, IRayCasterImpl,
    ITriggerVolumeComponent,
};
use crate::components::collision::{
    TriggerShape, TriggerVolumeComponent, bake_entity_collider, entity_world_xz_aabb,
};
use crate::math::{Transform, Vec3};
use crate::scene::CoreEntity;
use crate::utils::ray_casting::{AARayDirection, RayCaster};

pub struct CollisionWorldComponent {
    /// Aggregated world-space collider geometry (floor / wall / ...),
    /// shared (`Rc`) so [`WorldRayCaster`] can read it live.
    caster: Rc<RefCell<RayCaster>>,
    segment_triggers: RefCell<Vec<ComRc<ITriggerVolumeComponent>>>,
    proximity_triggers: RefCell<Vec<ComRc<ITriggerVolumeComponent>>>,
    /// Payload id of the first segment trigger crossed by the most
    /// recent [`evaluate_segment_triggers`](Self::evaluate_segment_triggers).
    fired_segment: Cell<Option<i64>>,
}

ComObject_CollisionWorldComponent!(super::CollisionWorldComponent);

impl CollisionWorldComponent {
    pub fn create() -> ComRc<crate::comdef::ICollisionWorldComponent> {
        ComRc::from_object(Self {
            caster: Rc::new(RefCell::new(RayCaster::new())),
            segment_triggers: RefCell::new(Vec::new()),
            proximity_triggers: RefCell::new(Vec::new()),
            fired_segment: Cell::new(None),
        })
    }

    /// Bake `entity`'s static-mesh subtree into the aggregated collider.
    /// World transforms are refreshed first so triangles are baked in
    /// world space.
    pub fn attach_collider(&self, entity: &ComRc<IEntity>) {
        entity.update_world_transform(&Transform::new());
        bake_entity_collider(&mut self.caster.borrow_mut(), entity);
    }

    /// Register a world-space segment-crossing event region from its
    /// explicit corners (8 → box, 4 → quad), carrying the opaque
    /// `id` / `tag` payload. The engine owns the holder entity, so the
    /// caller keeps no handle. A corner count other than 4 / 8 is
    /// ignored.
    pub fn attach_segment_trigger(&self, corners: Vec<Vec3>, id: i64, tag: String) {
        let holder = CoreEntity::create(format!("SegmentTrigger{}", id), true);
        if let Some(volume) =
            TriggerVolumeComponent::create(holder, TriggerShape::Segment(corners), id, tag)
        {
            self.segment_triggers.borrow_mut().push(volume);
        }
    }

    /// Register a proximity interaction probe on `entity`. The XZ
    /// rectangle is derived from the entity's mesh extent (falling back
    /// to its anchor), with `radius` slack; the volume tracks the
    /// entity's translation. The component is also attached to `entity`.
    pub fn attach_proximity_trigger(
        &self,
        entity: &ComRc<IEntity>,
        id: i64,
        tag: String,
        radius: f32,
    ) {
        entity.update_world_transform(&Transform::new());
        let shape = match entity_world_xz_aabb(entity) {
            Some(aabb) => TriggerShape::ProximityAabb { aabb, radius },
            None => TriggerShape::ProximityPoint {
                point: entity.world_transform().position(),
                radius,
            },
        };
        if let Some(volume) = TriggerVolumeComponent::create(entity.clone(), shape, id, tag) {
            entity.add_component(
                crate::comdef::ITriggerVolumeComponent::uuid(),
                volume.query_interface::<crate::comdef::IComponent>().unwrap(),
            );
            self.proximity_triggers.borrow_mut().push(volume);
        }
    }

    /// Scriptable `IRayCaster` over the aggregated collider, read live
    /// (colliders attached after this call are still visible).
    pub fn floor_ray_caster(&self) -> ComRc<IRayCaster> {
        WorldRayCaster::create(self.caster.clone())
    }

    /// Generic ray cast against the aggregated collider.
    pub fn cast_ray(&self, origin: &Vec3, direction: &Vec3) -> Option<f32> {
        self.caster.borrow().cast_ray(origin, direction)
    }

    /// Downward (`-Y`) floor cast against the aggregated collider.
    pub fn cast_aa_ny(&self, origin: &Vec3) -> Option<f32> {
        self.caster.borrow().cast_aaray(origin, AARayDirection::NY)
    }

    /// Evaluate every segment trigger against the movement segment
    /// `origin -> end`, latching per-volume fired flags and recording
    /// the first fired payload for [`fired_segment_trigger`](Self::fired_segment_trigger).
    pub fn evaluate_segment_triggers(&self, origin: &Vec3, end: &Vec3) {
        let mut fired = None;
        for volume in self.segment_triggers.borrow().iter() {
            let inner = volume.inner::<TriggerVolumeComponent>();
            inner.evaluate_segment(origin, end);
            if fired.is_none() && volume.triggered() {
                fired = Some(inner.id());
            }
        }
        self.fired_segment.set(fired);
    }

    /// Payload id of the segment trigger crossed by the most recent
    /// [`evaluate_segment_triggers`](Self::evaluate_segment_triggers).
    pub fn fired_segment_trigger(&self) -> Option<i64> {
        self.fired_segment.get()
    }

    /// The proximity trigger closest to `point` (XZ) within its own
    /// radius, or `None`.
    pub fn nearest_proximity_trigger(
        &self,
        point: &Vec3,
    ) -> Option<ComRc<ITriggerVolumeComponent>> {
        let mut min_distance = f32::INFINITY;
        let mut nearest = None;
        for volume in self.proximity_triggers.borrow().iter() {
            let inner = volume.inner::<TriggerVolumeComponent>();
            let dist = inner.distance_xz(point);
            if dist < inner.radius() && dist < min_distance {
                min_distance = dist;
                nearest = Some(volume.clone());
            }
        }
        nearest
    }
}

impl ICollisionWorldComponentImpl for CollisionWorldComponent {}

impl IComponentImpl for CollisionWorldComponent {
    fn on_loading(&self) -> crosscom::Void {}
    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {}
    fn on_unloading(&self) {}
}

/// Live `IRayCaster` view over a [`CollisionWorldComponent`]'s shared
/// aggregated caster.
pub struct WorldRayCaster {
    caster: Rc<RefCell<RayCaster>>,
}

ComObject_WorldRayCaster!(super::WorldRayCaster);

impl WorldRayCaster {
    pub fn create(caster: Rc<RefCell<RayCaster>>) -> ComRc<IRayCaster> {
        ComRc::from_object(Self { caster })
    }
}

impl IRayCasterImpl for WorldRayCaster {
    fn cast_aa_ny(&self, ox: f32, oy: f32, oz: f32) -> Option<f32> {
        self.caster
            .borrow()
            .cast_aaray(&Vec3::new(ox, oy, oz), AARayDirection::NY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::collision::CollisionWorldComponent;
    use crate::scene::CoreEntity;

    /// The world returns the nearest proximity trigger within its radius
    /// and reports its opaque payload, and returns `None` once the probe
    /// leaves every radius.
    #[test]
    fn nearest_proximity_trigger_picks_closest_in_radius() {
        let world_com = CollisionWorldComponent::create();
        let world = world_com.inner::<CollisionWorldComponent>();

        let near = CoreEntity::create("near".to_string(), true);
        near.transform()
            .borrow_mut()
            .set_position(&Vec3::new(0.0, 0.0, 0.0));
        near.update_world_transform(&Transform::new());
        world.attach_proximity_trigger(&near, 1, "near_fn".to_string(), 10.0);

        let far = CoreEntity::create("far".to_string(), true);
        far.transform()
            .borrow_mut()
            .set_position(&Vec3::new(100.0, 0.0, 0.0));
        far.update_world_transform(&Transform::new());
        world.attach_proximity_trigger(&far, 2, "far_fn".to_string(), 10.0);

        // Probe next to `near`: it wins.
        let hit = world
            .nearest_proximity_trigger(&Vec3::new(1.0, 0.0, 0.0))
            .expect("near volume should fire");
        assert_eq!(hit.inner::<TriggerVolumeComponent>().tag(), "near_fn");

        // Probe far from both (outside every radius): None.
        assert!(
            world
                .nearest_proximity_trigger(&Vec3::new(50.0, 0.0, 0.0))
                .is_none()
        );
    }
}
