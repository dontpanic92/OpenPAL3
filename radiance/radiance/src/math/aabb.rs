//! Axis-aligned bounding box utilities for frustum culling.
//!
//! Two helpers, kept tiny and dependency-free so the rendering engine
//! can fold the per-render-object visibility test into the existing
//! bucketing pass without reaching into any backend code:
//!
//! - [`transform_aabb`] maps a local-space AABB through a 4x4 affine /
//!   projective transform by transforming all eight corners and
//!   recomputing the (min, max) extent. This is the
//!   "transform-then-bound" method — slightly looser than computing
//!   the tight world-space AABB analytically, but free of edge cases
//!   for non-affine matrices (shears, projections) and good enough
//!   for the level of pre-cull rejection we want.
//!
//! - [`aabb_visible`] tests an AABB against a six-plane [`Frustum`]
//!   using the "n-vertex" trick: for each plane, pick the corner of
//!   the AABB that is most along the plane normal; if that corner is
//!   outside, the entire AABB is outside. False positives (returns
//!   `true` for a box that's actually fully outside) are possible
//!   only when the box straddles all six planes at corners that no
//!   single plane rules out — for the camera frustums this code
//!   sees, this is rare enough that we don't bother with the more
//!   expensive 8-corner SAT test.

use super::Mat44;
use crate::scene::Frustum;

/// Transform a local-space AABB `(min, max)` by `m` and return the
/// AABB of the eight transformed corners.
pub fn transform_aabb(min: [f32; 3], max: [f32; 3], m: &Mat44) -> ([f32; 3], [f32; 3]) {
    let f = m.floats();
    let xs = [min[0], max[0]];
    let ys = [min[1], max[1]];
    let zs = [min[2], max[2]];

    let mut out_min = [f32::INFINITY; 3];
    let mut out_max = [f32::NEG_INFINITY; 3];
    for &x in &xs {
        for &y in &ys {
            for &z in &zs {
                let wx = f[0][0] * x + f[0][1] * y + f[0][2] * z + f[0][3];
                let wy = f[1][0] * x + f[1][1] * y + f[1][2] * z + f[1][3];
                let wz = f[2][0] * x + f[2][1] * y + f[2][2] * z + f[2][3];
                if wx < out_min[0] {
                    out_min[0] = wx;
                }
                if wy < out_min[1] {
                    out_min[1] = wy;
                }
                if wz < out_min[2] {
                    out_min[2] = wz;
                }
                if wx > out_max[0] {
                    out_max[0] = wx;
                }
                if wy > out_max[1] {
                    out_max[1] = wy;
                }
                if wz > out_max[2] {
                    out_max[2] = wz;
                }
            }
        }
    }
    (out_min, out_max)
}

/// Test whether a world-space AABB `(min, max)` is at least partially
/// inside `frustum`. Returns `true` if the AABB might be visible (a
/// safe over-estimate); returns `false` only when it is provably
/// outside.
pub fn aabb_visible(min: [f32; 3], max: [f32; 3], frustum: &Frustum) -> bool {
    for plane in &frustum.planes {
        let (nx, ny, nz, d) = (plane[0], plane[1], plane[2], plane[3]);
        // p-vertex (the corner most in the direction of n). If even
        // the most-positive corner is on the negative side, the box
        // is fully outside this plane → reject.
        let px = if nx >= 0.0 { max[0] } else { min[0] };
        let py = if ny >= 0.0 { max[1] } else { min[1] };
        let pz = if nz >= 0.0 { max[2] } else { min[2] };
        if nx * px + ny * py + nz * pz + d < 0.0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Camera;

    fn box_at(c: [f32; 3], half: f32) -> ([f32; 3], [f32; 3]) {
        (
            [c[0] - half, c[1] - half, c[2] - half],
            [c[0] + half, c[1] + half, c[2] + half],
        )
    }

    #[test]
    fn transform_aabb_identity_returns_input() {
        let (mn, mx) = box_at([1.0, 2.0, 3.0], 0.5);
        let (omn, omx) = transform_aabb(mn, mx, &Mat44::new_identity());
        assert_eq!(mn, omn);
        assert_eq!(mx, omx);
    }

    #[test]
    fn aabb_visible_obvious_in_and_out() {
        // A standard camera looking down -Z from the origin (engine
        // default: identity transform, far_clip = 100000).
        let cam = Camera::new();
        let frustum = cam.frustum();

        // Point in front of the camera, well within near/far —
        // expected inside.
        let (mn, mx) = box_at([0.0, 0.0, -100.0], 1.0);
        assert!(aabb_visible(mn, mx, &frustum));

        // Point behind the camera — expected outside.
        let (mn, mx) = box_at([0.0, 0.0, 100.0], 1.0);
        assert!(!aabb_visible(mn, mx, &frustum));

        // Point far to the side at the near plane — expected outside.
        let (mn, mx) = box_at([10000.0, 0.0, -50.0], 1.0);
        assert!(!aabb_visible(mn, mx, &frustum));
    }
}
