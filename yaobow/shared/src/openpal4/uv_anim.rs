//! PAL4 water UV animation driver.
//!
//! Pairs a `_water.dff` entity with one or more `UvAnim` entries from
//! the scene's sibling `.uva` `UvAnimDict` and ticks the resulting
//! UV-affine transform (`uv_scale` / `uv_offset`) into the rendering
//! pipeline each frame.
//!
//! **Strict matching**: only render objects whose source material has
//! an exact name match in the `UvAnimDict` are mutated. PAL4 water
//! DFFs commonly bundle non-animated decoration (pond bed, banks,
//! lotus pads using a generic auto-named material) alongside the
//! animated water surfaces; the UVA dict explicitly lists only the
//! ones authored to animate.
//!
//! ## Required infrastructure
//!
//! Two renderer-side prerequisites have to be in place for this driver
//! to be correct:
//!
//! 1. **Per-object descriptor binding** — `SwapChain::draw_groups`
//!    must bind each draw's own material params descriptor, not a
//!    group-representative material's. Otherwise mutating one water
//!    material's UBO leaks the UV transform onto every other render
//!    object that shares the same `MaterialKey` (program/blend/depth/
//!    cull) — which in PAL4 is essentially every textured opaque
//!    mesh.
//! 2. **Material cache opt-out** — water DFFs are loaded with
//!    `DffLoaderConfig::force_unique_materials = true`, which stamps
//!    each `MaterialDef` with a fresh nonce via `MaterialDef::make_unique`
//!    so the `MaterialIdentity` cache produces a distinct
//!    `VulkanMaterial` per water material, never shared with a
//!    matching non-water material.
//!
//! The keyframe-decode step is heuristic — see [`decode_keyframes`].

use std::collections::HashMap;

use crosscom::ComRc;
use fileformats::rwbs::uva::{UvAnim, UvAnimDict, UV_ANIM_VUB_MAGIC};
use radiance::comdef::IEntity;

/// One decoded UV keyframe.
#[derive(Copy, Clone, Debug)]
struct UvKey {
    scale: [f32; 2],
    offset: [f32; 2],
}

impl UvKey {
    const fn identity() -> Self {
        Self {
            scale: [1.0, 1.0],
            offset: [0.0, 0.0],
        }
    }
}

/// One named animation in the dictionary — looked up by material name.
#[derive(Clone, Debug)]
struct NamedAnim {
    keys: [UvKey; 2],
    period: f32,
}

impl NamedAnim {
    fn sample(&self, t: f32) -> ([f32; 2], [f32; 2]) {
        // Linear interp through the cycle: alpha grows 0 → 1 across the
        // animation's duration, then snaps back to 0 at the wrap. Water
        // textures use REPEAT addressing and the keyframe deltas are
        // typically integer multiples of the texture period (e.g.
        // ty = -28 between kf0 and kf1 in Q01), so the sawtooth jump at
        // wrap is visually seamless.
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

/// One scene binding: an entity whose render objects should be
/// inspected each tick. Render objects whose `material_debug_name`
/// matches a key in `anims_by_material` receive that animation's
/// time-driven UV xform; all others stay static.
struct UvAnimBinding {
    entity: ComRc<IEntity>,
    anims_by_material: HashMap<String, NamedAnim>,
}

/// Per-scene driver. Lives on `Pal4Scene`; ticked once per frame by the
/// PAL4 app-context update loop.
#[derive(Default)]
pub struct UvAnimDriver {
    bindings: Vec<UvAnimBinding>,
    elapsed: f32,
}

impl UvAnimDriver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a water entity with the full UV-animation dictionary.
    ///
    /// Every animation in `dict` is decoded once up-front. At tick time,
    /// the driver walks the entity tree, calls
    /// `RenderObject::material_debug_name()` on each render object, and
    /// applies the matching animation only when an exact name match is
    /// found. Render objects whose material isn't listed stay static.
    pub fn register_water_entity(&mut self, entity: ComRc<IEntity>, dict: &UvAnimDict) {
        if dict.animations.is_empty() {
            log::warn!("[uv-anim] register_water_entity: dict has 0 animations; nothing to bind");
            return;
        }

        let mut anims_by_material = HashMap::new();
        for a in &dict.animations {
            let keys = decode_keyframes(a);
            let period = if a.duration > 0.0 { a.duration } else { 2.0 };
            log::debug!(
                "[uv-anim]   binding material '{}' duration={:.3}s kf0=(scale={:?}, offset={:?}) kf1=(scale={:?}, offset={:?})",
                a.name,
                a.duration,
                keys[0].scale,
                keys[0].offset,
                keys[1].scale,
                keys[1].offset,
            );
            anims_by_material.insert(a.name.clone(), NamedAnim { keys, period });
        }

        self.bindings.push(UvAnimBinding {
            entity,
            anims_by_material,
        });
    }

    pub fn tick(&mut self, delta_sec: f32) {
        if self.bindings.is_empty() {
            return;
        }
        self.elapsed += delta_sec;
        for b in &self.bindings {
            apply_uv_xform_recursive(&b.entity, self.elapsed, &b.anims_by_material);
        }
    }
}

/// Recursively walk the entity tree. For each `RenderObject` whose
/// `material_debug_name` is in `anims`, apply the matching animation's
/// interpolated UV transform. All other render objects (and non-Vulkan
/// backends whose `RenderObject::set_uv_xform` is a no-op) are left
/// untouched.
fn apply_uv_xform_recursive(
    entity: &ComRc<IEntity>,
    t: f32,
    anims: &HashMap<String, NamedAnim>,
) {
    if let Some(rc) = entity.get_rendering_component() {
        for obj in rc.render_objects() {
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

/// Decode the two keyframes of a `UvAnim`'s 96-byte `raw_keyframes` blob.
///
/// **Heuristic** (no authoritative spec exists for this PAL4 variant):
///
/// * Find the first occurrence of the `"VU\x05B"` magic in the blob.
/// * Treat the 28 bytes immediately *after* it as keyframe 1 (7 × f32).
/// * Treat the 28 bytes immediately *before* it as keyframe 0 (7 × f32).
/// * Within each 7-float record, interpret the layout as
///   `[?, sx, sy, ?, tx, ty, ?]`. The three `?` slots are zero in every
///   bundled sample and are ignored.
/// * Clamp degenerate scales (`|s| < 1e-6`, which catches encoded `-0`)
///   to `1.0` so the keyframe doesn't collapse the surface.
///
/// Falls back to identity on parse failure so non-water materials and
/// unexpected blobs stay benign.
fn decode_keyframes(anim: &UvAnim) -> [UvKey; 2] {
    let raw = &anim.raw_keyframes;
    let target = UV_ANIM_VUB_MAGIC.to_le_bytes();
    let mut kf0 = UvKey::identity();
    let mut kf1 = UvKey::identity();

    let mut found_at: Option<usize> = None;
    for i in 0..raw.len().saturating_sub(4) {
        if raw[i..i + 4] == target {
            found_at = Some(i);
            break;
        }
    }

    if let Some(i) = found_at {
        if i + 4 + 28 <= raw.len() {
            kf1 = parse_uv_frame(&raw[i + 4..i + 4 + 28]);
        }
        if i >= 28 {
            kf0 = parse_uv_frame(&raw[i - 28..i]);
        }
    }

    [kf0, kf1]
}

fn parse_uv_frame(b: &[u8]) -> UvKey {
    debug_assert!(b.len() >= 28);
    let f = |off: usize| f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]);
    let mut sx = f(4);
    let mut sy = f(8);
    let tx = f(16);
    let ty = f(20);
    if sx.abs() < 1e-6 {
        sx = 1.0;
    }
    if sy.abs() < 1e-6 {
        sy = 1.0;
    }
    UvKey {
        scale: [sx, sy],
        offset: [tx, ty],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_anim_from_raw(raw: Vec<u8>) -> UvAnim {
        UvAnim {
            version: 0x100,
            type_id: 0x1C1,
            num_frames: 2,
            flags: 0,
            duration: 0.0,
            name: "test".to_string(),
            raw_keyframes: raw,
        }
    }

    #[test]
    fn decode_finds_keyframes_around_vub_magic() {
        // Build a 96-byte raw blob with the BJ_water.uva shape:
        // pad...frame0(7f + sentinel)...VUB-magic...frame1(7f + tail).
        let mut raw = vec![0u8; 96];
        // frame 0 at rk[36..63]: 7 floats [_, 0, 1, _, 0, 0, _] plus
        // sentinel at [60..63].
        let kf0_floats = [0.0f32, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        for (i, v) in kf0_floats.iter().enumerate() {
            raw[36 + i * 4..36 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        // VUB magic at rk[64..67].
        raw[64..68].copy_from_slice(&UV_ANIM_VUB_MAGIC.to_le_bytes());
        // frame 1 at rk[68..95]: [_, 1, 1, _, -1, 1, _].
        let kf1_floats = [0.0f32, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0];
        for (i, v) in kf1_floats.iter().enumerate() {
            raw[68 + i * 4..68 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }

        let keys = decode_keyframes(&synth_anim_from_raw(raw));
        // kf0 has sx=0 → clamped to 1; sy=1; offset (0, 0).
        assert_eq!(keys[0].scale, [1.0, 1.0]);
        assert_eq!(keys[0].offset, [0.0, 0.0]);
        // kf1: sx=1, sy=1, tx=-1, ty=1.
        assert_eq!(keys[1].scale, [1.0, 1.0]);
        assert_eq!(keys[1].offset, [-1.0, 1.0]);
    }

    #[test]
    fn decode_returns_identity_when_no_magic() {
        let keys = decode_keyframes(&synth_anim_from_raw(vec![0u8; 96]));
        assert_eq!(keys[0].scale, [1.0, 1.0]);
        assert_eq!(keys[1].scale, [1.0, 1.0]);
        assert_eq!(keys[0].offset, [0.0, 0.0]);
        assert_eq!(keys[1].offset, [0.0, 0.0]);
    }

    #[test]
    fn sample_sawtooth_lerps_linearly_across_period() {
        let a = NamedAnim {
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
