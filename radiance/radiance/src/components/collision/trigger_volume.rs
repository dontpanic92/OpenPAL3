//! [`TriggerVolumeComponent`] — a generic trigger region attached to an
//! entity, carrying an opaque game payload.
//!
//! Two shapes cover the games' needs:
//!
//! * [`TriggerShape::Segment`] — an explicit world-space convex mesh
//!   (an 8-corner box or 4-corner quad). Fired by testing whether a
//!   movement *segment* (this frame's `origin -> end`) crosses the
//!   mesh. This is PAL4's EVF event-region model.
//! * [`TriggerShape::Proximity`] — a horizontal (XZ) axis-aligned
//!   rectangle (degenerate to a point for anchor-only volumes) with a
//!   slack `radius`. The rectangle follows the owning entity's
//!   translation, so a proximity probe stays correct after the entity
//!   is moved by script — replacing PAL4's hand-cached
//!   `objects_xz_aabbs` + manual invalidation hooks. Queried on demand
//!   (e.g. on a key press) via [`TriggerVolumeComponent::distance_xz`].
//!
//! The payload (`id` + `tag`) is opaque: PAL4 stores the EVF event
//! index in `id` and the GOB research-function name in `tag`; other
//! games assign their own meaning.

use std::cell::Cell;

use crosscom::ComRc;

use crate::comdef::{IComponentImpl, IEntity, IEntityExt, ITriggerVolumeComponentImpl};
use crate::math::Vec3;
use crate::utils::ray_casting::RayCaster;

lazy_static::lazy_static! {
    /// Triangle indices for an 8-corner trigger box (EVF box region).
    static ref BOX_TRIGGER_INDICES: Vec<u32> = vec![
        0, 2, 1, 0, 3, 2, 0, 4, 7, 0, 7, 3, 0, 5, 4, 0, 1, 5, 6, 1, 2, 6, 5, 1, 6, 2, 3, 6, 3,
        7, 6, 7, 4, 6, 4, 5,
    ];

    /// Triangle indices for a 4-corner trigger quad (EVF plane region).
    static ref PLANE_TRIGGER_INDICES: Vec<u32> = vec![0, 1, 2, 2, 1, 3];
}

/// A horizontal (XZ) proximity rectangle that follows its owning
/// entity's translation. A point anchor is the degenerate case
/// `min == max`.
#[derive(Clone, Copy, Debug)]
struct Proximity {
    /// World-space XZ rectangle captured at construction.
    base_min_x: f32,
    base_max_x: f32,
    base_min_z: f32,
    base_max_z: f32,
    /// Owning entity's world position at construction. The rectangle is
    /// shifted by `current_position - origin` at query time so it
    /// tracks translation (`giGOBMovment` / `giGOBSetPosition`) without
    /// any external cache invalidation.
    origin: Vec3,
    /// Slack radius added on top of the rectangle when deciding whether
    /// a probe point counts as "inside".
    radius: f32,
}

enum Shape {
    /// World-space convex mesh tested by segment crossing.
    Segment(RayCaster),
    /// XZ proximity rectangle (+ slack radius).
    Proximity(Proximity),
}

/// Build-time shape descriptor handed to
/// [`TriggerVolumeComponent::create`].
pub enum TriggerShape {
    /// Explicit world-space corners: 8 → box, 4 → quad. Tested by
    /// segment crossing.
    Segment(Vec<Vec3>),
    /// World-space XZ rectangle `(min_x, max_x, min_z, max_z)` with a
    /// slack `radius`. The rectangle follows the entity's translation.
    ProximityAabb {
        aabb: (f32, f32, f32, f32),
        radius: f32,
    },
    /// World-space anchor point with a slack `radius`. Follows the
    /// entity's translation.
    ProximityPoint { point: Vec3, radius: f32 },
}

pub struct TriggerVolumeComponent {
    entity: ComRc<IEntity>,
    shape: Shape,
    id: i64,
    tag: String,
    triggered: Cell<bool>,
}

ComObject_TriggerVolumeComponent!(super::TriggerVolumeComponent);

impl TriggerVolumeComponent {
    /// Attach a trigger volume to `entity` with an opaque `id` / `tag`
    /// payload. Returns `None` for a `Segment` shape whose corner count
    /// is neither 4 nor 8.
    pub fn create(
        entity: ComRc<IEntity>,
        shape: TriggerShape,
        id: i64,
        tag: String,
    ) -> Option<ComRc<crate::comdef::ITriggerVolumeComponent>> {
        let origin = entity.world_transform().position();
        let shape = match shape {
            TriggerShape::Segment(corners) => {
                let mut ray_caster = RayCaster::new();
                match corners.len() {
                    4 => ray_caster.add_mesh(corners, PLANE_TRIGGER_INDICES.clone()),
                    8 => ray_caster.add_mesh(corners, BOX_TRIGGER_INDICES.clone()),
                    _ => return None,
                }
                Shape::Segment(ray_caster)
            }
            TriggerShape::ProximityAabb {
                aabb: (min_x, max_x, min_z, max_z),
                radius,
            } => Shape::Proximity(Proximity {
                base_min_x: min_x,
                base_max_x: max_x,
                base_min_z: min_z,
                base_max_z: max_z,
                origin,
                radius,
            }),
            TriggerShape::ProximityPoint { point, radius } => Shape::Proximity(Proximity {
                base_min_x: point.x,
                base_max_x: point.x,
                base_min_z: point.z,
                base_max_z: point.z,
                origin,
                radius,
            }),
        };

        Some(ComRc::from_object(Self {
            entity,
            shape,
            id,
            tag,
            triggered: Cell::new(false),
        }))
    }

    /// Opaque numeric payload (PAL4: the EVF event index).
    pub fn id(&self) -> i64 {
        self.id
    }

    /// Opaque string payload (PAL4: the GOB research-function name).
    pub fn tag(&self) -> String {
        self.tag.clone()
    }

    /// Slack radius for a proximity volume (`0.0` for segment volumes).
    pub fn radius(&self) -> f32 {
        match &self.shape {
            Shape::Proximity(p) => p.radius,
            Shape::Segment(_) => 0.0,
        }
    }

    /// Does the movement segment `origin -> end` cross this volume?
    /// Always `false` for proximity volumes.
    pub fn intersects_segment(&self, origin: &Vec3, end: &Vec3) -> bool {
        match &self.shape {
            Shape::Segment(ray_caster) => {
                let direction = Vec3::sub(end, origin);
                // `cast_ray` returns the parametric distance `t` along
                // `direction` (hit = origin + t * direction); the segment
                // crosses the volume this frame iff `t <= 1.0`. The lower
                // bound (`t > EPSILON`) is enforced by `cast_ray`.
                match ray_caster.cast_ray(origin, &direction) {
                    Some(t) => t <= 1.0,
                    None => false,
                }
            }
            Shape::Proximity(_) => false,
        }
    }

    /// Evaluate the movement segment and latch the result into the
    /// per-frame [`triggered`](Self::triggered) flag.
    pub fn evaluate_segment(&self, origin: &Vec3, end: &Vec3) {
        self.triggered.set(self.intersects_segment(origin, end));
    }

    /// Horizontal (XZ) distance from `point` to this volume's proximity
    /// rectangle (after tracking the entity's translation), ignoring Y.
    /// Returns `f32::INFINITY` for segment volumes. The closest point on
    /// the rectangle is used, so a point inside it yields `0.0`.
    pub fn distance_xz(&self, point: &Vec3) -> f32 {
        let p = match &self.shape {
            Shape::Proximity(p) => p,
            Shape::Segment(_) => return f32::INFINITY,
        };

        // Shift the captured rectangle by the entity's translation since
        // construction so it tracks `giGOBMovment` / `giGOBSetPosition`.
        let pos = self.entity.world_transform().position();
        let dx_off = pos.x - p.origin.x;
        let dz_off = pos.z - p.origin.z;
        let min_x = p.base_min_x + dx_off;
        let max_x = p.base_max_x + dx_off;
        let min_z = p.base_min_z + dz_off;
        let max_z = p.base_max_z + dz_off;

        let dx = if point.x < min_x {
            min_x - point.x
        } else if point.x > max_x {
            point.x - max_x
        } else {
            0.0
        };
        let dz = if point.z < min_z {
            min_z - point.z
        } else if point.z > max_z {
            point.z - max_z
        } else {
            0.0
        };
        (dx * dx + dz * dz).sqrt()
    }
}

impl ITriggerVolumeComponentImpl for TriggerVolumeComponent {
    fn triggered(&self) -> bool {
        self.triggered.get()
    }
}

impl IComponentImpl for TriggerVolumeComponent {
    fn on_loading(&self) -> crosscom::Void {}
    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {}
    fn on_unloading(&self) {}
}
