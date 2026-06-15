//! Generic engine-backed collision & trigger components.
//!
//! These promote what used to be per-game (PAL4) trigger / collision
//! code into reusable `radiance` ECS components attached to entities:
//!
//! * [`CollisionMeshComponent`] turns any entity carrying static mesh
//!   geometry into a ray-castable collider (floor / wall nav-meshes,
//!   scenery, ...). It folds in the world-space triangle bake the games
//!   previously hand-rolled.
//! * [`TriggerVolumeComponent`] models a trigger region attached to an
//!   entity — an explicit world-space convex mesh (segment-crossing
//!   event regions) or an entity-local AABB / sphere (proximity
//!   interaction probes) — carrying an opaque, game-assigned payload.
//!
//! Both are plain [`IComponent`](crate::comdef::IComponent)s, so the
//! owning scene ticks them every frame; the game assigns meaning to the
//! payloads and drives evaluation through the components' query
//! primitives.

mod collision_mesh;
mod collision_world;
mod trigger_volume;

pub use collision_mesh::{CollisionMeshComponent, bake_entity_collider};
pub use collision_world::{CollisionWorldComponent, WorldRayCaster};
pub use trigger_volume::{TriggerShape, TriggerVolumeComponent};

use crosscom::ComRc;

use crate::comdef::{IEntity, IEntityExt, IStaticMeshComponent};
use crate::components::mesh::static_mesh::StaticMeshComponent;
use crate::math::{Mat44, Vec3};

/// Multiply a row-major 4x4 affine matrix by a point `(x, y, z, 1)`
/// and return the resulting 3D point. Shared by the collider bake and
/// the trigger-volume world-space transforms so geometry is seen in
/// world space regardless of which child entity it originated from.
pub(crate) fn transform_point(m: &Mat44, p: &Vec3) -> Vec3 {
    Vec3::new(
        m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
        m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
        m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
    )
}

/// Walk an entity tree and return the world-space XZ axis-aligned
/// bounding box `(min_x, max_x, min_z, max_z)` across every vertex of
/// every `IStaticMeshComponent` found, or `None` when the tree has no
/// static mesh. Used to seed a proximity [`TriggerVolumeComponent`] so
/// an interaction probe fires when the player is next to the visible
/// mesh, regardless of where the entity's anchor sits relative to it.
pub fn entity_world_xz_aabb(entity: &ComRc<IEntity>) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    accumulate_xz_aabb(entity, &mut min_x, &mut max_x, &mut min_z, &mut max_z);
    if min_x.is_finite() && max_x.is_finite() && max_x >= min_x && max_z >= min_z {
        Some((min_x, max_x, min_z, max_z))
    } else {
        None
    }
}

fn accumulate_xz_aabb(
    entity: &ComRc<IEntity>,
    min_x: &mut f32,
    max_x: &mut f32,
    min_z: &mut f32,
    max_z: &mut f32,
) {
    for child in entity.children() {
        accumulate_xz_aabb(&child, min_x, max_x, min_z, max_z);
    }

    if let Some(mesh) = entity.get_component(IStaticMeshComponent::uuid()) {
        let mesh = mesh.query_interface::<IStaticMeshComponent>().unwrap();
        let world_matrix = *entity.world_transform().matrix();
        let mesh_inner = mesh.inner::<StaticMeshComponent>();
        let geometries = mesh_inner.get_geometries();
        for geometry in geometries.iter() {
            for v in geometry.vertices.to_position_vec() {
                let w = transform_point(&world_matrix, &v);
                if w.x < *min_x {
                    *min_x = w.x;
                }
                if w.x > *max_x {
                    *max_x = w.x;
                }
                if w.z < *min_z {
                    *min_z = w.z;
                }
                if w.z > *max_z {
                    *max_z = w.z;
                }
            }
        }
    }
}
