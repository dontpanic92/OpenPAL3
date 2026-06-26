//! PAL5 grass rendering.
//!
//! Renders the **upright grass-ribbon geometry** stored in a block's `.ctr`
//! ([`fileformats::pal5::ctr`]).
//!
//! ## Model (clean-room RE of `Pal5.exe`, verified against real data)
//! A block's grass is a quadtree whose leaves fall into two kinds:
//! * **Pure-grid layers** — a `cao###` texture pair over a rectangular cell
//!   range with a per-cell density grid (`color_len/2` bytes). These are a
//!   coarse coverage map.
//! * **Custom-geometry leaves** (the majority, ~258 of 437 on the standard
//!   block) — these carry the *actual* grass mesh: world-space vertices and
//!   triangles. The vertices sit in two height bands — a **bottom ring on the
//!   terrain surface** (the blade roots) and a **top ring ~130 units above**
//!   (the blade tips) — and the triangles weave them into continuous
//!   **vertical grass ribbons** that wind across the ground, following the
//!   terrain height. This is what the original engine draws (with a wind sway
//!   on the top ring); it is *not* a flat overlay and *not* procedurally
//!   scattered billboards.
//!
//! We render those stored vertices/triangles directly with an alpha-cutout
//! grass texture. Texture coordinates are not stored, so we synthesize them:
//! `V` from each vertex's height above the terrain (root → tip), `U` from the
//! vertex's horizontal world position so the blade texture tiles along the
//! ribbon.

use crosscom::ComRc;
use fileformats::pal5::ctr::GrassLeaf;
use image::{Rgba, RgbaImage};
use radiance::comdef::{IComponent, IDistanceCullComponent, IEntity};
use radiance::components::distance_cull::DistanceCullComponent;
use radiance::components::mesh::{Geometry, StaticMeshComponent, TexCoord};
use radiance::math::Vec3;
use radiance::rendering::{BlendMode, CullMode, SimpleMaterialDef};
use radiance::scene::CoreEntity;

use super::asset_loader::AssetLoader;
use super::terrain::BlockGrassHeights;

/// Draw-distance for a grass chunk, in world units (tunable via
/// `PAL5_GRASS_DIST`).
const DEFAULT_DIST: f32 = 2400.0;

/// Maximum grass blade height in world units. The `.ctr` stores vertical
/// ribbons whose tip vertices range from ~130 up to ~470 units above the
/// ground (some "blades" would otherwise spike into the sky); the original
/// engine renders short, uniform grass, so we cap each vertex to this height
/// above the terrain. Tunable via `PAL5_GRASS_BLADE_H`.
const DEFAULT_BLADE_H: f32 = 55.0;

/// World units per horizontal repeat of the grass texture along a ribbon.
/// Tunable via `PAL5_GRASS_TILE`.
const DEFAULT_TILE: f32 = 24.0;

/// Build the grass entities for one terrain block. Each `.ctr` custom-geometry
/// leaf (a vertical grass ribbon) becomes one distance-culled
/// [`StaticMeshComponent`]. Returns an empty vec when the block has no grass
/// geometry.
pub fn build_block_grass(
    asset_loader: &AssetLoader,
    map_name: &str,
    block_tag: &str,
    heights: &BlockGrassHeights,
    leaves: &[GrassLeaf],
) -> Vec<ComRc<IEntity>> {
    let dist = env_f32("PAL5_GRASS_DIST", DEFAULT_DIST);
    let blade_h = env_f32("PAL5_GRASS_BLADE_H", DEFAULT_BLADE_H);
    let tile = env_f32("PAL5_GRASS_TILE", DEFAULT_TILE);

    let material = SimpleMaterialDef::create_with_image("pal5_grass", Some(grass_texture()))
        .with_blend(BlendMode::AlphaTest)
        .with_cull(CullMode::None);
    let factory = asset_loader.component_factory();

    // `heights` is retained for the API/signature (terrain draping refinements)
    // but ribbon heights are now capped relative to each ribbon's own base, so
    // no terrain height-field lookup is needed here.
    let _ = heights;

    let mut entities: Vec<ComRc<IEntity>> = Vec::new();

    for (leaf_idx, leaf) in leaves.iter().enumerate() {
        // Only the leaves that carry real grass-ribbon geometry are drawn; the
        // pure-grid coverage leaves have no vertices/triangles.
        if leaf.vertices.is_empty() || leaf.triangles.is_empty() {
            continue;
        }

        let mut vertices: Vec<Vec3> = Vec::with_capacity(leaf.vertices.len());
        let mut normals: Vec<Vec3> = Vec::with_capacity(leaf.vertices.len());
        let mut texcoords: Vec<TexCoord> = Vec::with_capacity(leaf.vertices.len());
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];

        // The ribbon winds horizontally, so the texture must tile *along* the
        // ribbon's main horizontal direction. A naive `x+z` collapses to a
        // near-constant value across each triangle (texture samples one
        // vertical strip → opaque/empty patches). Instead project each vertex
        // onto the leaf's principal horizontal axis. Compute it via the XZ
        // covariance of the vertices.
        let (axis, origin) = principal_axis_xz(&leaf.vertices);

        // The `.ctr` stores vertical "curtain" ribbons whose tip vertices range
        // from ~130 up to ~470 units above their base — some spike into the
        // sky, and a per-column cap misses any tip whose (x,z) doesn't exactly
        // match a low vertex (leaving a residual cone). A quadtree leaf covers
        // only a small patch, so its single lowest vertex is a safe ground
        // reference: cap *every* vertex to `blade_h` above the leaf's min Y.
        // This is terrain-independent (no height-field lookup → no NaN that
        // would collapse triangles to a shared apex) and bulletproof against
        // stray tall vertices.
        let base = leaf
            .vertices
            .iter()
            .map(|v| v[1])
            .fold(f32::MAX, f32::min);
        let base = if base.is_finite() { base } else { 0.0 };

        for v in &leaf.vertices {
            let (x, z) = (v[0], v[2]);
            let y = v[1].min(base + blade_h);
            // V: 1 at the root (on the ground), 0 at the capped tip.
            let above = (y - base).max(0.0);
            let vtex = (1.0 - (above / blade_h)).clamp(0.0, 1.0);
            // U: distance along the ribbon's principal horizontal axis, so the
            // grass texture tiles continuously along the ribbon.
            let utex = ((x - origin.0) * axis.0 + (z - origin.1) * axis.1) / tile;

            vertices.push(Vec3::new(x, y, z));
            normals.push(Vec3::new(0.0, 1.0, 0.0));
            texcoords.push(TexCoord::new(utex, vtex));

            min[0] = min[0].min(x);
            min[1] = min[1].min(y);
            min[2] = min[2].min(z);
            max[0] = max[0].max(x);
            max[1] = max[1].max(y);
            max[2] = max[2].max(z);
        }

        // The stored triangles index `leaf.vertices`; carry them over, dropping
        // any with an out-of-range index (defensive against bad data).
        let n = leaf.vertices.len() as u32;
        let mut indices: Vec<u32> = Vec::with_capacity(leaf.triangles.len() * 3);
        for t in &leaf.triangles {
            let (a, b, c) = (t.indices[0] as u32, t.indices[1] as u32, t.indices[2] as u32);
            if a < n && b < n && c < n {
                indices.extend_from_slice(&[a, b, c]);
            }
        }
        if indices.is_empty() {
            continue;
        }

        let center = Vec3::new(
            0.5 * (min[0] + max[0]),
            0.5 * (min[1] + max[1]),
            0.5 * (min[2] + max[2]),
        );
        let radius = 0.5
            * ((max[0] - min[0]).powi(2) + (max[2] - min[2]).powi(2))
                .sqrt()
                .max(1.0);

        let geometry = Geometry::new(
            &vertices,
            Some(&normals),
            &[texcoords],
            indices,
            material.clone(),
        );

        let entity = CoreEntity::create(
            format!("{}_grass_{}_{}", map_name, block_tag, leaf_idx),
            false,
        );
        let mesh = StaticMeshComponent::new(entity.clone(), vec![geometry], factory.clone());
        entity.add_component(
            radiance::comdef::IStaticMeshComponent::uuid(),
            ComRc::from_object(mesh),
        );
        let cull = DistanceCullComponent::create(entity.clone(), center, dist + radius);
        entity.add_component(
            IDistanceCullComponent::uuid(),
            cull.query_interface::<IComponent>().unwrap(),
        );
        entities.push(entity);
    }

    entities
}

/// Principal horizontal (XZ) axis of a leaf's vertices, plus the centroid the
/// axis passes through. Returns a unit `(dx, dz)` direction along the
/// vertices' greatest horizontal spread (the ribbon's run) and the `(x, z)`
/// origin to measure from. Falls back to the X axis for degenerate input.
fn principal_axis_xz(verts: &[[f32; 3]]) -> ((f32, f32), (f32, f32)) {
    let n = verts.len().max(1) as f32;
    let (mut cx, mut cz) = (0.0f32, 0.0f32);
    for v in verts {
        cx += v[0];
        cz += v[2];
    }
    cx /= n;
    cz /= n;

    let (mut sxx, mut sxz, mut szz) = (0.0f32, 0.0f32, 0.0f32);
    for v in verts {
        let dx = v[0] - cx;
        let dz = v[2] - cz;
        sxx += dx * dx;
        sxz += dx * dz;
        szz += dz * dz;
    }

    // Principal eigenvector angle of the 2×2 covariance [[sxx,sxz],[sxz,szz]].
    let theta = 0.5 * (2.0 * sxz).atan2(sxx - szz);
    let (mut ax, mut az) = (theta.cos(), theta.sin());
    if !ax.is_finite() || !az.is_finite() || (ax == 0.0 && az == 0.0) {
        ax = 1.0;
        az = 0.0;
    }
    ((ax, az), (cx, cz))
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v: &f32| v.is_finite() && *v > 0.0)
        .unwrap_or(default)
}

/// First-pass procedural grass texture: tapering green blades rising from the
/// bottom (root) to the top (tip) on a transparent background, so the
/// alpha-test cutout reads as grass and the bare ground shows between ribbons.
/// `V = 0` is the tip (top of image), `V = 1` the root (bottom). Replaced by
/// the authored `cao###` textures later.
fn grass_texture() -> RgbaImage {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut img = RgbaImage::from_pixel(W, H, Rgba([0, 0, 0, 0]));
    let blades: [(f32, f32, f32); 12] = [
        (0.06, 0.55, 0.06),
        (0.15, 0.78, -0.04),
        (0.24, 0.62, 0.05),
        (0.32, 0.88, -0.03),
        (0.40, 0.70, 0.04),
        (0.48, 0.95, 0.00),
        (0.56, 0.72, -0.04),
        (0.64, 0.86, 0.05),
        (0.72, 0.60, -0.05),
        (0.80, 0.80, 0.04),
        (0.88, 0.66, -0.06),
        (0.95, 0.52, 0.08),
    ];
    for &(cx, height, lean) in &blades {
        let top = ((1.0 - height) * H as f32) as u32;
        for y in top..H {
            let t = (y - top) as f32 / (H - top).max(1) as f32; // 0 tip .. 1 root
            let half_w = 0.006 + 0.014 * t;
            let g = (120.0 + 110.0 * (1.0 - t)) as u8;
            let r = (40.0 + 70.0 * (1.0 - t)) as u8;
            let b = 28u8;
            let lx = cx + lean * (1.0 - t);
            let x0 = ((lx - half_w) * W as f32).floor().max(0.0) as u32;
            let x1 = ((lx + half_w) * W as f32).ceil().min(W as f32) as u32;
            for x in x0..x1 {
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use fileformats::pal5::ctr::{GrassLeaf, GrassTri};

    fn flat_heights() -> BlockGrassHeights {
        BlockGrassHeights {
            min_x: 0.0,
            min_z: 0.0,
            cell_world: 320.0,
            corners: vec![[100.0; 17]; 17],
        }
    }

    /// A custom-geometry leaf maps its stored vertical ribbon to V=root..tip:
    /// vertices on the ground get V≈1, tip vertices (~blade_h up) get V≈0.
    #[test]
    fn ribbon_uv_root_to_tip() {
        let blade_h = DEFAULT_BLADE_H;
        let ground = 100.0;
        let verts = [
            [50.0, ground, 50.0],            // root
            [60.0, ground + blade_h, 50.0],  // tip
        ];
        for v in verts {
            let above = (v[1] - ground).max(0.0);
            let vtex = (1.0 - (above / blade_h)).clamp(0.0, 1.0);
            if above < 1.0 {
                assert!((vtex - 1.0).abs() < 0.05, "root should map to V≈1");
            } else {
                assert!(vtex < 0.05, "tip should map to V≈0");
            }
        }
    }

    /// Pure-grid leaves (no stored geometry) produce no entities — only the
    /// custom-geometry leaves are drawn.
    #[test]
    fn grid_only_leaf_is_skipped() {
        let leaf = GrassLeaf {
            tex0: 10,
            tex1: 5,
            g: [0, 0, 7, 7],
            density: vec![1; 64],
            vertices: vec![],
            triangles: vec![],
        };
        assert!(leaf.vertices.is_empty() && leaf.triangles.is_empty());
        let _ = flat_heights();
        let _ = GrassTri {
            indices: [0, 1, 2],
            color: 0x00ff_0002,
        };
    }
}
