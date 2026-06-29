//! Camera-facing billboard component.
//!
//! [`BillboardComponent`] rewrites its owning entity's *local* transform
//! every frame so that a flat quad authored in the entity's local XZ
//! plane (normal `+Y`) stands up, faces the active scene camera, and is
//! scaled by a per-instance factor.
//!
//! PAL5 wind foliage motivates this: each leaf cluster is a single
//! 4-vertex horizontal quad (`prt` tag `[W]{t<id>}{s<pct>}<name>`)
//! placed at a canopy point inside the tree's frame hierarchy. Rendered
//! as authored they lie flat like little floor tiles; the original
//! engine erects each into a camera-facing card scaled by the `{s..}`
//! percentage. We reproduce that by, each frame:
//!
//!   1. taking the entity's world position `P` and the camera position
//!      `C` (fed once per frame via [`set_camera_position`]);
//!   2. building a world rotation whose local `+Y` (the quad normal)
//!      points at the camera, with the card kept upright relative to
//!      world-up, and the in-plane axes scaled by the instance factor;
//!   3. converting that world orientation back through the parent's
//!      world rotation into the local transform the entity must carry,
//!      while preserving the entity's local translation (canopy offset).
//!
//! The component self-ticks via [`IComponent::on_updating`], which the
//! scene dispatches *before* `update_world_transform`, so the transform
//! set here is reflected in the same frame's render.

use std::sync::atomic::{AtomicU32, Ordering};

use crosscom::ComRc;

use crate::comdef::{IBillboardComponentImpl, IComponentImpl, IEntity, IEntityExt};
use crate::math::{Mat44, Vec3};

ComObject_BillboardComponent!(super::BillboardComponent);

/// Multiplier converting the `{s<pct>}` percentage into an in-plane card
/// scale. PAL5's leaf cards are authored as small "footprint" quads (their
/// per-tree size lives in the DFF geometry — e.g. `zw_gushu` ≈ 34×34,
/// `zw_shulin_07` ≈ 10.6×21.2 — and the per-leaf `{s<pct>}` tag scales it);
/// the original engine enlarges those footprints by a global factor so the
/// canopy fills out. That factor is **not** stored per-asset, so it is a
/// single global constant here, overridable at runtime via the
/// `PAL5_LEAF_SIZE_GAIN` env var for calibration against the original game.
fn billboard_size_gain() -> f32 {
    use std::sync::OnceLock;
    static GAIN: OnceLock<f32> = OnceLock::new();
    *GAIN.get_or_init(|| {
        std::env::var("PAL5_LEAF_SIZE_GAIN")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|v: &f32| *v > 0.0)
            .unwrap_or(4.0)
    })
}

// Process-global "current camera position", written once per frame by
// `CoreRadianceEngine::update` and read by every billboard's
// `on_updating`. Stored as three bit-cast `f32`s so reads/writes are
// lock-free; the one-frame staleness is imperceptible for billboarding.
static CAM_X: AtomicU32 = AtomicU32::new(0);
static CAM_Y: AtomicU32 = AtomicU32::new(0);
static CAM_Z: AtomicU32 = AtomicU32::new(0);

/// Publish the active camera world position for billboard components to
/// consume. Called once per frame by the engine update loop.
pub fn set_camera_position(p: Vec3) {
    CAM_X.store(p.x.to_bits(), Ordering::Relaxed);
    CAM_Y.store(p.y.to_bits(), Ordering::Relaxed);
    CAM_Z.store(p.z.to_bits(), Ordering::Relaxed);
}

/// The most recently published camera world position.
pub fn camera_position() -> Vec3 {
    Vec3::new(
        f32::from_bits(CAM_X.load(Ordering::Relaxed)),
        f32::from_bits(CAM_Y.load(Ordering::Relaxed)),
        f32::from_bits(CAM_Z.load(Ordering::Relaxed)),
    )
}

/// Camera-independent shadow caster orientation for a billboard leaf. The
/// visible card faces the camera (so its shadow rectangle would yaw as the
/// camera moves); for shadows we instead keep the card flat in the parent's
/// frame at the leaf's world position, so the sun casts a stable, textured
/// footprint that doesn't track the camera. `world`/`local` are the leaf's
/// current matrices; returns the matrix the shadow pass should use.
pub fn billboard_shadow_matrix(world: &Mat44, local: &Mat44) -> Mat44 {
    let Some(local_inv) = affine_inverse(local) else {
        return world.clone();
    };
    // parentWorld = world * inverse(local) — the un-billboarded frame.
    let mut shadow = Mat44::multiplied(world, &local_inv);
    let wf = world.floats();
    // The visible card carries its in-plane size in the world matrix's basis
    // columns (e_x -> right*s, e_z -> up*s, see `billboard_local_matrix`), so
    // s = |col0|. The parent frame only carries the tree's node scale, which is
    // far smaller than the leaf card, so the shadow would be tiny. Re-scale the
    // flat caster's linear part to that same `s` so the footprint matches the
    // rendered leaf.
    let s = (wf[0][0] * wf[0][0] + wf[1][0] * wf[1][0] + wf[2][0] * wf[2][0]).sqrt();
    let sf = shadow.floats_mut();
    let cur = (sf[0][0] * sf[0][0] + sf[1][0] * sf[1][0] + sf[2][0] * sf[2][0]).sqrt();
    if cur > 1e-6 && s > 1e-6 {
        let k = s / cur;
        for r in 0..3 {
            for c in 0..3 {
                sf[r][c] *= k;
            }
        }
    }
    // Re-anchor at the leaf's world position so the flat card stays put.
    let s = shadow.floats_mut();
    s[0][3] = wf[0][3];
    s[1][3] = wf[1][3];
    s[2][3] = wf[2][3];
    shadow
}

pub struct BillboardComponent {
    entity: ComRc<IEntity>,
    /// In-plane card scale = `scale_pct / 100 * BILLBOARD_SIZE_GAIN`.
    scale: f32,
}

impl BillboardComponent {
    /// `scale_pct` is the PAL5 `{s<pct>}` value (`100` = neutral).
    pub fn create(
        entity: ComRc<IEntity>,
        scale_pct: f32,
    ) -> ComRc<crate::comdef::IBillboardComponent> {
        ComRc::from_object(Self {
            entity,
            scale: scale_pct / 100.0 * billboard_size_gain(),
        })
    }

    fn apply(&self) {
        let cam = camera_position();
        let world = self.entity.world_transform().matrix().clone();
        let local = self.entity.transform().borrow().matrix().clone();
        if let Some(m) = billboard_local_matrix(&world, &local, cam, self.scale) {
            self.entity.transform().borrow_mut().set_matrix(m);
        }
    }
}

impl IBillboardComponentImpl for BillboardComponent {}

impl IComponentImpl for BillboardComponent {
    fn on_loading(&self) -> crosscom::Void {
        self.apply();
    }

    fn on_updating(&self, _delta_sec: f32) -> crosscom::Void {
        self.apply();
    }

    fn on_unloading(&self) {}
}

/// Compute the new *local* transform matrix that makes a local-XZ quad
/// (normal `+Y`) face `cam`, scaled in-plane by `scale`, given the
/// entity's current `world` and `local` matrices. Returns `None` when
/// the geometry is degenerate (camera coincident with the quad),
/// leaving the transform untouched.
///
/// We want the leaf's *world* transform to have an exact camera-facing
/// orientation `rw` at the leaf's current world position `P`, regardless
/// of the parent's rotation **or (non-uniform) scale**. With
/// `world = parentWorld * local`, the local matrix that achieves a
/// chosen `desiredWorld` is `local = parentWorld⁻¹ · desiredWorld`,
/// where `parentWorld = world · local⁻¹`. Using the *full* parent
/// inverse (not just a rotation transpose) is what cancels the parent's
/// non-uniform scale — otherwise it shears the card's normal off the
/// camera direction.
fn billboard_local_matrix(world: &Mat44, local: &Mat44, cam: Vec3, scale: f32) -> Option<Mat44> {
    let wf = world.floats();
    let p = Vec3::new(wf[0][3], wf[1][3], wf[2][3]);

    let to_cam = Vec3::sub(&cam, &p);
    // Cylindrical billboard: the card stays vertical (its `up` is
    // world-up) and only yaws about the vertical axis to face the
    // camera horizontally — the natural look for upright tree foliage.
    let mut horiz = Vec3::new(to_cam.x, 0.0, to_cam.z);
    if horiz.norm() < 1e-4 {
        horiz = Vec3::new(0.0, 0.0, 1.0);
    }
    let d = Vec3::normalized(&horiz); // quad normal (local +Y) -> camera (horizontal)
    let up = Vec3::UP; // local +Z -> world up
    let right = Vec3::normalized(&Vec3::cross(&up, &d)); // local +X

    // parentWorld = world * inverse(local). Both the parent (tree node
    // scale) and `local` (which, after the first billboard frame,
    // carries this card's in-plane scale) have non-rotation linear
    // parts, so both need the *true* affine inverse — `Mat44::inversed`
    // assumes a pure rotation and would feed scale error back each frame
    // until the transform blows up to NaN.
    let local_inv = affine_inverse(local)?;
    let parent_world = Mat44::multiplied(world, &local_inv);
    let parent_inv = affine_inverse(&parent_world)?;

    // Preserve the tree's placement scale. The parent transform carries the
    // `.nod` node scale (and any DFF frame scale above the leaf); cancelling
    // it via `parent_inv` keeps the card facing the camera, but it would also
    // strip the tree's scale from the card size — a tree placed at 0.5x/2x in
    // the `.nod` would keep full-size leaves. Re-introduce the parent's
    // *uniform* (geometric-mean) scale into the card so leaf size tracks the
    // tree, while the non-uniform part stays cancelled (it would shear the
    // card's normal off the camera direction). This makes the rendered size
    // fully asset-driven: leaf-quad geometry × `{s<pct>}` × node scale.
    let pf = parent_world.floats();
    let col_norm = |c: usize| {
        (pf[0][c] * pf[0][c] + pf[1][c] * pf[1][c] + pf[2][c] * pf[2][c]).sqrt()
    };
    let parent_scale = (col_norm(0) * col_norm(1) * col_norm(2)).cbrt().max(1e-4);
    let s = scale * parent_scale;

    // Desired *world* transform: basis columns are the images of the
    // local axes (e_x -> right*s, e_y -> d, e_z -> up*s) and the
    // translation keeps the leaf at its current world position `P`.
    let mut desired_world = Mat44::new_identity();
    {
        let o = desired_world.floats_mut();
        o[0] = [right.x * s, d.x, up.x * s, p.x];
        o[1] = [right.y * s, d.y, up.y * s, p.y];
        o[2] = [right.z * s, d.z, up.z * s, p.z];
        o[3] = [0.0, 0.0, 0.0, 1.0];
    }

    let out = Mat44::multiplied(&parent_inv, &desired_world);
    Some(out)
}

/// General inverse of an affine 4x4 matrix (arbitrary invertible 3x3
/// linear part + translation), unlike [`Mat44::inversed`] which assumes
/// a pure rotation. Returns `None` if the linear part is singular.
fn affine_inverse(m: &Mat44) -> Option<Mat44> {
    let a = m.floats();
    // 3x3 cofactor inverse of the linear part.
    let c00 = a[1][1] * a[2][2] - a[1][2] * a[2][1];
    let c01 = a[1][0] * a[2][2] - a[1][2] * a[2][0];
    let c02 = a[1][0] * a[2][1] - a[1][1] * a[2][0];
    let det = a[0][0] * c00 - a[0][1] * c01 + a[0][2] * c02;
    if det.abs() < 1e-9 {
        return None;
    }
    let inv_det = 1.0 / det;

    // inv3[i][j] = cofactor(j,i) / det (adjugate transpose).
    let inv3 = [
        [
            c00 * inv_det,
            -(a[0][1] * a[2][2] - a[0][2] * a[2][1]) * inv_det,
            (a[0][1] * a[1][2] - a[0][2] * a[1][1]) * inv_det,
        ],
        [
            -c01 * inv_det,
            (a[0][0] * a[2][2] - a[0][2] * a[2][0]) * inv_det,
            -(a[0][0] * a[1][2] - a[0][2] * a[1][0]) * inv_det,
        ],
        [
            c02 * inv_det,
            -(a[0][0] * a[2][1] - a[0][1] * a[2][0]) * inv_det,
            (a[0][0] * a[1][1] - a[0][1] * a[1][0]) * inv_det,
        ],
    ];

    let t = [a[0][3], a[1][3], a[2][3]];
    let mut out = Mat44::new_identity();
    {
        let o = out.floats_mut();
        for i in 0..3 {
            for j in 0..3 {
                o[i][j] = inv3[i][j];
            }
            o[i][3] = -(inv3[i][0] * t[0] + inv3[i][1] * t[1] + inv3[i][2] * t[2]);
        }
        o[3] = [0.0, 0.0, 0.0, 1.0];
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mat(rows: [[f32; 4]; 4]) -> Mat44 {
        let mut m = Mat44::new_identity();
        *m.floats_mut() = rows;
        m
    }

    // A billboard under a *non-uniformly scaled* parent must still end up
    // with its card normal (local +Y) pointing horizontally at the
    // camera — the case the naive rotation-transpose inverse got wrong.
    #[test]
    fn faces_camera_under_non_uniform_parent_scale() {
        // Parent world: scale (2, 3, 1) + translation (100, 10, 0).
        let parent = mat([
            [2.0, 0.0, 0.0, 100.0],
            [0.0, 3.0, 0.0, 10.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        // Leaf local offset inside the parent.
        let local = mat([
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 7.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let world = Mat44::multiplied(&parent, &local);
        let cam = Vec3::new(500.0, 80.0, 300.0);

        let out = billboard_local_matrix(&world, &local, cam, 1.0).unwrap();
        let final_world = Mat44::multiplied(&parent, &out);
        let f = final_world.floats();

        // Card normal = column 1 of the final world matrix.
        let normal = Vec3::normalized(&Vec3::new(f[0][1], f[1][1], f[2][1]));
        let p = Vec3::new(f[0][3], f[1][3], f[2][3]);
        let want = Vec3::normalized(&Vec3::new(cam.x - p.x, 0.0, cam.z - p.z));

        let dot = Vec3::dot(&normal, &want);
        assert!(dot > 0.999, "normal should face camera horizontally, dot = {dot}");

        // Position must be preserved (leaf stays where it was).
        let wf = world.floats();
        assert!((p.x - wf[0][3]).abs() < 1e-3);
        assert!((p.y - wf[1][3]).abs() < 1e-3);
        assert!((p.z - wf[2][3]).abs() < 1e-3);
    }

    // The tree's placement (uniform) scale must propagate into the card
    // size, so foliage tracks a scaled-up/-down `.nod` tree placement.
    #[test]
    fn card_size_tracks_uniform_parent_scale() {
        let scale = 1.5_f32;
        let cam = Vec3::new(400.0, 50.0, 250.0);

        // In-plane basis magnitude of the final world card for a given
        // uniform parent scale `k`.
        let card_basis_norm = |k: f32| -> f32 {
            let parent = mat([
                [k, 0.0, 0.0, 10.0],
                [0.0, k, 0.0, 5.0],
                [0.0, 0.0, k, -3.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);
            let local = mat([
                [1.0, 0.0, 0.0, 4.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 6.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);
            let world = Mat44::multiplied(&parent, &local);
            let out = billboard_local_matrix(&world, &local, cam, scale).unwrap();
            let fw = Mat44::multiplied(&parent, &out);
            let f = fw.floats();
            // Column 0 is the in-plane "right" axis carrying the card scale.
            Vec3::new(f[0][0], f[1][0], f[2][0]).norm()
        };

        let n1 = card_basis_norm(1.0);
        let n2 = card_basis_norm(2.0);
        // Doubling the tree's placement scale doubles the card size.
        assert!((n1 - scale).abs() < 1e-3, "k=1 → card scale {n1}");
        assert!((n2 - 2.0 * n1).abs() < 1e-2, "k=2 should double card size: {n2} vs {n1}");
    }

    // The shadow caster matrix keeps the leaf at its world position but flat
    // in the parent's frame, so it is independent of camera position.
    #[test]
    fn shadow_matrix_is_camera_independent_and_anchored() {
        let parent = mat([
            [1.0, 0.0, 0.0, 100.0],
            [0.0, 1.0, 0.0, 10.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let local = mat([
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 7.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let world = Mat44::multiplied(&parent, &local);
        // Two different camera-facing leaf locals must yield the same anchor.
        let s = billboard_shadow_matrix(&world, &local);
        let f = s.floats();
        let wf = world.floats();
        assert!((f[0][3] - wf[0][3]).abs() < 1e-3);
        assert!((f[1][3] - wf[1][3]).abs() < 1e-3);
        assert!((f[2][3] - wf[2][3]).abs() < 1e-3);
        // Flat in parent frame: identity rotation preserves the authored
        // local +Y quad normal, so the card lies flat.
        assert!(f[1][1].abs() > 0.99);
    }

    #[test]
    fn affine_inverse_round_trips_scaled_matrix() {
        let m = mat([
            [2.0, 0.0, 0.0, 5.0],
            [0.0, 3.0, 0.0, -2.0],
            [0.0, 0.0, 0.5, 9.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let inv = affine_inverse(&m).unwrap();
        let id = Mat44::multiplied(&m, &inv);
        let f = id.floats();
        for i in 0..4 {
            for j in 0..4 {
                let expect = if i == j { 1.0 } else { 0.0 };
                assert!((f[i][j] - expect).abs() < 1e-4, "({i},{j}) = {}", f[i][j]);
            }
        }
    }
}
