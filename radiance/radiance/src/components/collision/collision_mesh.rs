//! [`CollisionMeshComponent`] — a generic ray-castable collider baked
//! from an entity's static mesh geometry.

use std::rc::Rc;

use crosscom::ComRc;

use crate::comdef::{
    ICollisionMeshComponentImpl, IComponentImpl, IEntity, IEntityExt, IStaticMeshComponent,
};
use crate::components::collision::transform_point;
use crate::components::mesh::static_mesh::StaticMeshComponent;
use crate::math::Vec3;
use crate::utils::ray_casting::{AARayDirection, RayCaster};

/// Walk `entity` and its children, baking every `IStaticMeshComponent`
/// triangle into `ray_caster` in **world space** (the entity's full
/// world transform — translation + rotation + scale — is folded into
/// each vertex). Callers must ensure world transforms are current
/// (e.g. via `IEntity::update_world_transform`) before baking.
///
/// Hoisted out of PAL4's `openpal4::scene` so any game can turn an
/// entity subtree into a collider.
pub fn bake_entity_collider(ray_caster: &mut RayCaster, entity: &ComRc<IEntity>) {
    for child in entity.children() {
        bake_entity_collider(ray_caster, &child);
    }

    if let Some(mesh) = entity.get_component(IStaticMeshComponent::uuid()) {
        let mesh = mesh.query_interface::<IStaticMeshComponent>().unwrap();
        let world_matrix = *entity.world_transform().matrix();
        let mesh_inner = mesh.inner::<StaticMeshComponent>();
        let geometries = mesh_inner.get_geometries();
        for geometry in geometries.iter() {
            let v = geometry
                .vertices
                .to_position_vec()
                .into_iter()
                .map(|v| transform_point(&world_matrix, &v))
                .collect();
            let i = geometry.indices.clone();
            ray_caster.add_mesh(v, i);
        }
    }
}

/// Engine-backed collider attached to an entity. Bakes the entity
/// subtree's static geometry into a shared [`RayCaster`] at construction
/// and exposes ray-cast queries (generic + downward floor cast).
pub struct CollisionMeshComponent {
    ray_caster: Rc<RayCaster>,
    _entity: ComRc<IEntity>,
}

ComObject_CollisionMeshComponent!(super::CollisionMeshComponent);

impl CollisionMeshComponent {
    /// Build a collider by baking `entity`'s static-mesh subtree into a
    /// fresh world-space [`RayCaster`]. The caller is responsible for
    /// ensuring the subtree's world transforms are up to date.
    pub fn create(entity: ComRc<IEntity>) -> ComRc<crate::comdef::ICollisionMeshComponent> {
        let mut ray_caster = RayCaster::new();
        bake_entity_collider(&mut ray_caster, &entity);
        Self::from_ray_caster(entity, Rc::new(ray_caster))
    }

    /// Build a collider around an already-baked [`RayCaster`]. Lets
    /// callers compose colliders from several entities (e.g. PAL4's
    /// floor + wall pair) into one component.
    pub fn from_ray_caster(
        entity: ComRc<IEntity>,
        ray_caster: Rc<RayCaster>,
    ) -> ComRc<crate::comdef::ICollisionMeshComponent> {
        ComRc::from_object(Self {
            ray_caster,
            _entity: entity,
        })
    }

    /// Shared handle to the underlying caster, so callers can hand it to
    /// the scriptable `wrap_ray_caster` CCW without re-baking.
    pub fn ray_caster(&self) -> Rc<RayCaster> {
        self.ray_caster.clone()
    }
}

impl ICollisionMeshComponentImpl for CollisionMeshComponent {
    fn cast_ray(
        &self,
        ox: f32,
        oy: f32,
        oz: f32,
        dx: f32,
        dy: f32,
        dz: f32,
    ) -> Option<f32> {
        self.ray_caster
            .cast_ray(&Vec3::new(ox, oy, oz), &Vec3::new(dx, dy, dz))
    }

    fn cast_aa_ny(&self, ox: f32, oy: f32, oz: f32) -> Option<f32> {
        self.ray_caster
            .cast_aaray(&Vec3::new(ox, oy, oz), AARayDirection::NY)
    }
}

impl IComponentImpl for CollisionMeshComponent {
    fn on_loading(&self) -> crosscom::Void {}
    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {}
    fn on_unloading(&self) {}
}
