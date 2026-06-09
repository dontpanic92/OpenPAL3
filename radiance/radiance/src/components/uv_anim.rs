//! Generic UV-animation component.
//!
//! Drives a per-frame UV-affine transform (`uv_scale` / `uv_offset`)
//! on the render objects of its owning entity whose source material has
//! an exact name match in the animation set it was built with. All
//! other render objects (and non-Vulkan backends whose
//! [`RenderObject::set_uv_xform`] is a no-op) are left untouched.
//!
//! The component **self-ticks**: the owning container (scene / entity)
//! dispatches `IComponent::on_updating` every frame while it is active,
//! so callers only have to attach it once — no external driver holds or
//! ticks it.
//!
//! The keyframe specs ([`MaterialUvAnim`]) are decoded by the caller
//! (e.g. a game-specific `.uva` decoder) and handed in at construction;
//! this component is format-agnostic.
//!
//! ## Required renderer infrastructure
//!
//! Two renderer-side prerequisites must be in place for per-material UV
//! mutation to be correct:
//!
//! 1. **Per-object descriptor binding** — each draw must bind its own
//!    material params descriptor, not a group-representative material's,
//!    or mutating one material's UBO leaks the UV transform onto every
//!    other render object sharing the same `MaterialKey`.
//! 2. **Material cache opt-out** — animated DFFs are loaded with
//!    `DffLoaderConfig::force_unique_materials = true` so the material
//!    cache produces a distinct `VulkanMaterial` per animated material,
//!    never shared with a matching static material.

use std::cell::Cell;
use std::collections::HashMap;

use crosscom::ComRc;

use crate::comdef::{IComponentImpl, IEntity, IEntityExt};

/// One UV keyframe: an affine `(scale, offset)` applied to texture
/// coordinates.
#[derive(Copy, Clone, Debug)]
pub struct UvKey {
    pub scale: [f32; 2],
    pub offset: [f32; 2],
}

impl UvKey {
    pub const fn identity() -> Self {
        Self {
            scale: [1.0, 1.0],
            offset: [0.0, 0.0],
        }
    }
}

/// A two-keyframe UV animation, sampled as a linear sawtooth across
/// `period` seconds. Looked up by material debug-name.
#[derive(Clone, Debug)]
pub struct MaterialUvAnim {
    pub keys: [UvKey; 2],
    pub period: f32,
}

impl MaterialUvAnim {
    /// Linear interp through the cycle: alpha grows 0 → 1 across the
    /// animation's duration, then snaps back to 0 at the wrap. Texture
    /// addressing is REPEAT and keyframe deltas are typically integer
    /// multiples of the texture period, so the sawtooth jump at wrap is
    /// visually seamless.
    pub fn sample(&self, t: f32) -> ([f32; 2], [f32; 2]) {
        let alpha = (t / self.period.max(0.001)).fract();
        let lerp = |a: f32, b: f32| a + (b - a) * alpha;
        let s = [
            lerp(self.keys[0].scale[0], self.keys[1].scale[0]),
            lerp(self.keys[0].scale[1], self.keys[1].scale[1]),
        ];
        let o = [
            lerp(self.keys[0].offset[0], self.keys[1].offset[0]),
            lerp(self.keys[0].offset[1], self.keys[1].offset[1]),
        ];
        (s, o)
    }
}

pub struct UvAnimationComponent {
    entity: ComRc<IEntity>,
    anims_by_material: HashMap<String, MaterialUvAnim>,
    elapsed: Cell<f32>,
}

ComObject_UvAnimationComponent!(super::UvAnimationComponent);

impl UvAnimationComponent {
    pub fn create(
        entity: ComRc<IEntity>,
        anims_by_material: HashMap<String, MaterialUvAnim>,
    ) -> ComRc<crate::comdef::IUvAnimationComponent> {
        ComRc::from_object(Self {
            entity,
            anims_by_material,
            elapsed: Cell::new(0.0),
        })
    }
}

impl IComponentImpl for UvAnimationComponent {
    fn on_loading(&self) -> crosscom::Void {}

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {
        if self.anims_by_material.is_empty() {
            return;
        }
        let t = self.elapsed.get() + delta_sec;
        self.elapsed.set(t);
        apply_uv_xform_recursive(&self.entity, t, &self.anims_by_material);
    }

    fn on_unloading(&self) {}
}

/// Recursively walk the entity tree. For each `RenderObject` whose
/// `material_debug_name` is in `anims`, apply the matching animation's
/// interpolated UV transform. All other render objects are left
/// untouched.
fn apply_uv_xform_recursive(
    entity: &ComRc<IEntity>,
    t: f32,
    anims: &HashMap<String, MaterialUvAnim>,
) {
    if let Some(rc) = entity.get_rendering_component() {
        for obj in rc.render_objects() {
            let obj = obj.as_dyn();
            let Some(name) = obj.material_debug_name() else {
                continue;
            };
            let Some(anim) = anims.get(name) else {
                continue;
            };
            let (scale, offset) = anim.sample(t);
            obj.set_uv_xform(scale, offset);
        }
    }
    for child in entity.children() {
        apply_uv_xform_recursive(&child, t, anims);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_sawtooth_lerps_linearly_across_period() {
        let a = MaterialUvAnim {
            keys: [
                UvKey {
                    scale: [1.0, 1.0],
                    offset: [0.0, 0.0],
                },
                UvKey {
                    scale: [2.0, 2.0],
                    offset: [-1.0, 1.0],
                },
            ],
            period: 2.0,
        };
        // At t=0 alpha=0 → kf0
        let (s, o) = a.sample(0.0);
        assert_eq!(s, [1.0, 1.0]);
        assert_eq!(o, [0.0, 0.0]);
        // At t=1.0 (mid-period) alpha=0.5 → midpoint
        let (s, o) = a.sample(1.0);
        assert_eq!(s, [1.5, 1.5]);
        assert_eq!(o, [-0.5, 0.5]);
        // At t=2.0 (full period) alpha=0 (wraps to start)
        let (s, o) = a.sample(2.0);
        assert_eq!(s, [1.0, 1.0]);
        assert_eq!(o, [0.0, 0.0]);
        // Just before wrap, alpha ≈ 1 → near kf1
        let (s, o) = a.sample(1.99);
        assert!((s[0] - 1.995).abs() < 1e-3, "s[0]={}", s[0]);
        assert!((o[0] - (-0.995)).abs() < 1e-3, "o[0]={}", o[0]);
    }
}
