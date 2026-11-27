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
use crate::math::{Mat44, Transform, Vec3};
use crate::scene::CoreEntity;
use crate::utils::ray_casting::{AARayDirection, RayCaster};

pub struct CollisionWorldComponent {
    /// Aggregated world-space collider geometry (floor / wall / ...),
    /// shared (`Rc`) so [`WorldRayCaster`] can read it live.
    caster: Rc<RefCell<RayCaster>>,
    dynamic_colliders: Rc<RefCell<Vec<DynamicCollider>>>,
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
            dynamic_colliders: Rc::new(RefCell::new(Vec::new())),
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

    /// Register a rigid mesh subtree whose root transform may change.
    /// Unlike static floor geometry, disabled dynamic entities do not collide.
    pub fn attach_dynamic_collider(&self, entity: &ComRc<IEntity>) -> bool {
        entity.update_world_transform(&Transform::new());
        let mut caster = RayCaster::new();
        bake_entity_collider(&mut caster, entity);
        if caster.is_empty() {
            return false;
        }
        let mut colliders = self.dynamic_colliders.borrow_mut();
        if colliders
            .iter()
            .any(|collider| collider.entity.ptr_value() == entity.ptr_value())
        {
            return true;
        }
        colliders.push(DynamicCollider {
            entity: entity.clone(),
            pose: Cell::new(*entity.world_transform().matrix()),
            caster: RefCell::new(caster),
        });
        true
    }

    pub fn dynamic_floor_hit(&self, origin: &Vec3) -> Option<(ComRc<IEntity>, f32)> {
        self.dynamic_colliders
            .borrow()
            .iter()
            .filter_map(|collider| {
                collider
                    .cast_ray(origin, &Vec3::new(0., -1., 0.))
                    .map(|depth| (collider.entity.clone(), depth))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
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
                volume
                    .query_interface::<crate::comdef::IComponent>()
                    .unwrap(),
            );
            self.proximity_triggers.borrow_mut().push(volume);
        }
    }

    /// Scriptable `IRayCaster` over the aggregated collider, read live
    /// (colliders attached after this call are still visible).
    pub fn floor_ray_caster(&self) -> ComRc<IRayCaster> {
        ComRc::from_object(WorldRayCaster {
            caster: self.caster.clone(),
            dynamic_colliders: self.dynamic_colliders.clone(),
        })
    }

    /// Generic ray cast against the aggregated collider.
    pub fn cast_ray(&self, origin: &Vec3, direction: &Vec3) -> Option<f32> {
        self.caster
            .borrow()
            .cast_ray(origin, direction)
            .into_iter()
            .chain(
                self.dynamic_colliders
                    .borrow()
                    .iter()
                    .filter_map(|collider| collider.cast_ray(origin, direction)),
            )
            .min_by(f32::total_cmp)
    }

    /// Downward (`-Y`) floor cast against the aggregated collider.
    pub fn cast_aa_ny(&self, origin: &Vec3) -> Option<f32> {
        self.caster
            .borrow()
            .cast_aaray(origin, AARayDirection::NY)
            .into_iter()
            .chain(
                self.dynamic_colliders
                    .borrow()
                    .iter()
                    .filter_map(|collider| collider.cast_ray(origin, &Vec3::new(0., -1., 0.))),
            )
            .min_by(f32::total_cmp)
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
    dynamic_colliders: Rc<RefCell<Vec<DynamicCollider>>>,
}

struct DynamicCollider {
    entity: ComRc<IEntity>,
    pose: Cell<Mat44>,
    caster: RefCell<RayCaster>,
}

impl DynamicCollider {
    fn cast_ray(&self, origin: &Vec3, direction: &Vec3) -> Option<f32> {
        if !self.entity.enabled() {
            return None;
        }
        self.entity.update_world_transform(&Transform::new());
        let pose = *self.entity.world_transform().matrix();
        if pose.floats() != self.pose.get().floats() {
            let mut caster = RayCaster::new();
            bake_entity_collider(&mut caster, &self.entity);
            *self.caster.borrow_mut() = caster;
            self.pose.set(pose);
        }
        self.caster.borrow().cast_ray(origin, direction)
    }
}

ComObject_WorldRayCaster!(super::WorldRayCaster);

impl WorldRayCaster {
    pub fn create(caster: Rc<RefCell<RayCaster>>) -> ComRc<IRayCaster> {
        ComRc::from_object(Self {
            caster,
            dynamic_colliders: Rc::new(RefCell::new(Vec::new())),
        })
    }
}

impl IRayCasterImpl for WorldRayCaster {
    fn cast_aa_ny(&self, ox: f32, oy: f32, oz: f32) -> Option<f32> {
        let origin = Vec3::new(ox, oy, oz);
        self.caster
            .borrow()
            .cast_aaray(&origin, AARayDirection::NY)
            .into_iter()
            .chain(
                self.dynamic_colliders
                    .borrow()
                    .iter()
                    .filter_map(|collider| collider.cast_ray(&origin, &Vec3::new(0., -1., 0.))),
            )
            .min_by(f32::total_cmp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::collision::CollisionWorldComponent;
    use crate::rendering::{
        ComponentFactory, MaterialDef, RenderObjectHandle, RenderTarget, RenderingComponent,
        Texture, TextureDef, VertexBuffer, VideoPlayer,
    };
    use crate::scene::CoreEntity;
    use imgui::TextureId;

    struct UnrenderedFactory;

    impl ComponentFactory for UnrenderedFactory {
        fn create_texture(&self, _: &TextureDef) -> Box<dyn Texture> {
            unreachable!()
        }
        fn create_imgui_texture(
            &self,
            _: &[u8],
            _: u32,
            _: u32,
            _: u32,
            _: Option<TextureId>,
        ) -> (Box<dyn Texture>, TextureId) {
            unreachable!()
        }
        fn remove_imgui_texture(&self, _: Option<TextureId>) {
            unreachable!()
        }
        fn create_render_object(
            &self,
            _: VertexBuffer,
            _: Vec<u32>,
            _: &MaterialDef,
            _: bool,
        ) -> RenderObjectHandle {
            unreachable!()
        }
        fn create_rendering_component(&self, _: Vec<RenderObjectHandle>) -> RenderingComponent {
            unreachable!()
        }
        fn create_video_player(&self) -> Box<VideoPlayer> {
            unreachable!()
        }
        fn create_render_target(&self, _: u32, _: u32) -> Box<dyn RenderTarget> {
            unreachable!()
        }
    }

    #[test]
    fn dynamic_floor_queries_track_translation_rotation_scale_and_enabled_state() {
        let world = CollisionWorldComponent::create();
        let world = world.inner::<CollisionWorldComponent>();
        let script = world.floor_ray_caster();
        let cube = crate::debug::create_box_entity(Rc::new(UnrenderedFactory));
        assert!(world.attach_dynamic_collider(&cube));
        assert_eq!(script.cast_aa_ny(0., 10., 0.), Some(7.));
        cube.transform()
            .borrow_mut()
            .set_position(&Vec3::new(100., 10., 0.));
        assert_eq!(script.cast_aa_ny(0., 10., 0.), None);
        assert_eq!(script.cast_aa_ny(100., 20., 0.), Some(7.));
        cube.transform()
            .borrow_mut()
            .scale_local(&Vec3::new(2., 2., 2.));
        assert_eq!(script.cast_aa_ny(100., 20., 0.), Some(4.));
        assert_eq!(script.cast_aa_ny(107., 20., 0.), None);
        cube.transform()
            .borrow_mut()
            .rotate_axis_angle_local(&Vec3::UP, std::f32::consts::FRAC_PI_4);
        assert!((script.cast_aa_ny(107., 20., 0.).unwrap() - 4.).abs() < 0.0001);
        let (entity, depth) = world.dynamic_floor_hit(&Vec3::new(100., 20., 0.)).unwrap();
        assert_eq!(entity.ptr_value(), cube.ptr_value());
        assert!((depth - 4.).abs() < 0.0001);
        cube.set_enabled(false);
        assert_eq!(script.cast_aa_ny(100., 20., 0.), None);
        assert!(world.dynamic_floor_hit(&Vec3::new(100., 20., 0.)).is_none());
        cube.set_enabled(true);
        assert!(script.cast_aa_ny(100., 20., 0.).is_some());
        assert!(!world.attach_dynamic_collider(&CoreEntity::create("empty".into(), true)));
    }

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
