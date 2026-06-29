//! PAL4 water UV-animation decoding.
//!
//! Decodes a `_water.uva` `UvAnimDict` into the engine-generic
//! [`MaterialUvAnim`] specs and attaches a self-ticking
//! [`radiance::components::uv_anim::UvAnimationComponent`] to the paired
//! `_water.dff` entity. Once attached, the engine drives the per-frame
//! UV transform automatically (the component's `on_updating` fires while
//! its scene is active) — there is no driver to hold or tick.
//!
//! **Strict matching**: only render objects whose source material has
//! an exact name match in the `UvAnimDict` are animated. PAL4 water
//! DFFs commonly bundle non-animated decoration (pond bed, banks, lotus
//! pads) alongside the animated water surfaces; the UVA dict explicitly
//! lists only the ones authored to animate.
//!
//! The keyframe-decode step is heuristic — see [`decode_keyframes`].

use std::collections::HashMap;

use crosscom::ComRc;
use fileformats::rwbs::uva::{UvAnim, UvAnimDict};
use radiance::comdef::{IComponent, IEntity, IUvAnimationComponent};
use radiance::components::uv_anim::{MaterialUvAnim, UvAnimationComponent, UvKey};

/// Decode a `_water.uva` dict and attach a self-ticking UV-animation
/// component to `entity`. Every animation in `dict` is decoded once
/// up-front into the engine-generic [`MaterialUvAnim`] spec, keyed by
/// material name. The engine then ticks the component each frame while
/// the entity's scene is active.
///
/// No-op (with a warning) when the dict carries no animations.
pub fn attach_uv_anim(entity: &ComRc<IEntity>, dict: &UvAnimDict) {
    if dict.animations.is_empty() {
        log::warn!("[uv-anim] attach_uv_anim: dict has 0 animations; nothing to bind");
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
        anims_by_material.insert(a.name.clone(), MaterialUvAnim { keys, period });
    }

    let component = UvAnimationComponent::create(entity.clone(), anims_by_material);
    entity.add_component(
        IUvAnimationComponent::uuid(),
        component.query_interface::<IComponent>().unwrap(),
    );
}

/// Decode the two keyframes of a `UvAnim`'s 96-byte `raw_keyframes` blob.
///
/// **Heuristic** (no authoritative spec exists for this PAL4 variant):
///
/// * The keyframe block embeds the animation's `duration` a second time
///   as a 4-byte float that sits *between* keyframe 0 and keyframe 1.
///   Find the first occurrence of that duration value in the blob.
/// * Treat the 28 bytes immediately *after* it as keyframe 1 (7 × f32).
/// * Treat the 28 bytes immediately *before* it as keyframe 0 (7 × f32).
/// * Within each 7-float record, interpret the layout as
///   `[?, sx, sy, ?, tx, ty, ?]`. The three `?` slots are zero in every
///   bundled sample and are ignored.
/// * Clamp degenerate scales (`|s| < 1e-6`, which catches encoded `-0`)
///   to `1.0` so the keyframe doesn't collapse the surface.
///
/// Earlier revisions searched for a fixed `"VU\x05B"` byte pattern
/// (`0x42055556` ≈ 33.333 s) as if it were a magic separator. It is not
/// magic — it is simply the duration the PAL4 water overlays happen to
/// use. Other overlays — notably the ZJM start-menu cloud "trans" layer —
/// use a different duration (100 s), so that fixed pattern was never
/// found and those animations decoded to identity (i.e. stayed static).
/// We now always search by the animation's own `duration`.
///
/// Falls back to identity when the duration can't be located (or is an
/// unusable all-zero value) so non-water materials and unexpected blobs
/// stay benign.
fn decode_keyframes(anim: &UvAnim) -> [UvKey; 2] {
    let raw = &anim.raw_keyframes;
    let mut kf0 = UvKey::identity();
    let mut kf1 = UvKey::identity();

    // The separator is the animation's own duration, re-encoded as a
    // little-endian f32 between the two keyframes. Skip an all-zero
    // duration, which would match the leading zero padding and
    // mis-locate the split.
    let duration_bits = anim.duration.to_bits();
    if duration_bits == 0 {
        return [kf0, kf1];
    }
    let target = duration_bits.to_le_bytes();

    let mut found_at: Option<usize> = None;
    for i in 0..raw.len().saturating_sub(4) {
        if raw[i..i + 4] == target[..] {
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

    // The PAL4 water overlays encode a 33.333 s duration; its little-
    // endian f32 bytes happen to spell "VU\x05B". Earlier code mistook
    // that for a magic separator — it is just the duration.
    const WATER_DURATION_BITS: u32 = 0x4205_5556;

    fn synth_anim_from_raw(raw: Vec<u8>) -> UvAnim {
        synth_anim_from_raw_with_duration(raw, f32::from_bits(WATER_DURATION_BITS))
    }

    fn synth_anim_from_raw_with_duration(raw: Vec<u8>, duration: f32) -> UvAnim {
        UvAnim {
            version: 0x100,
            type_id: 0x1C1,
            num_frames: 2,
            flags: 0,
            duration,
            name: "test".to_string(),
            raw_keyframes: raw,
        }
    }

    #[test]
    fn decode_finds_keyframes_around_duration_separator() {
        // Build a 96-byte raw blob with the BJ_water.uva shape:
        // pad...frame0(7f + sentinel)...duration...frame1(7f + tail).
        let mut raw = vec![0u8; 96];
        // frame 0 at rk[36..63]: 7 floats [_, 0, 1, _, 0, 0, _] plus
        // sentinel at [60..63].
        let kf0_floats = [0.0f32, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        for (i, v) in kf0_floats.iter().enumerate() {
            raw[36 + i * 4..36 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        // Duration separator at rk[64..67].
        raw[64..68].copy_from_slice(&WATER_DURATION_BITS.to_le_bytes());
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
    fn decode_returns_identity_when_no_duration() {
        let keys = decode_keyframes(&synth_anim_from_raw_with_duration(vec![0u8; 96], 0.0));
        assert_eq!(keys[0].scale, [1.0, 1.0]);
        assert_eq!(keys[1].scale, [1.0, 1.0]);
        assert_eq!(keys[0].offset, [0.0, 0.0]);
        assert_eq!(keys[1].offset, [0.0, 0.0]);
    }

    #[test]
    fn decode_finds_keyframes_by_nonwater_duration() {
        // ZJM_trans.uva shape: identical to the water layout but the
        // keyframe separator is the animation's actual duration (100 s),
        // not the 33.333 s the water overlays use. Regression test for
        // the ZJM start-menu cloud overlay that previously stayed static
        // because the old code only looked for the water duration bits.
        let duration = 100.0f32;
        let mut raw = vec![0u8; 96];
        // frame 0 at rk[36..63]: [_, 1, 1, _, 0, 0, _].
        let kf0_floats = [0.0f32, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        for (i, v) in kf0_floats.iter().enumerate() {
            raw[36 + i * 4..36 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        // Duration separator at rk[64..67].
        raw[64..68].copy_from_slice(&duration.to_le_bytes());
        // frame 1 at rk[68..95]: [_, 1, 1, _, -1, 0, _].
        let kf1_floats = [0.0f32, 1.0, 1.0, 0.0, -1.0, 0.0, 0.0];
        for (i, v) in kf1_floats.iter().enumerate() {
            raw[68 + i * 4..68 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }

        let keys = decode_keyframes(&synth_anim_from_raw_with_duration(raw, duration));
        assert_eq!(keys[0].scale, [1.0, 1.0]);
        assert_eq!(keys[0].offset, [0.0, 0.0]);
        assert_eq!(keys[1].scale, [1.0, 1.0]);
        // The cloud layer scrolls the U offset from 0 → -1 across the
        // period; this is what was missing while it rendered static.
        assert_eq!(keys[1].offset, [-1.0, 0.0]);
    }
}
