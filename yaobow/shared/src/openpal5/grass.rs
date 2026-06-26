//! PAL5 grass rendering.
//!
//! Renders short, grounded, alpha-cutout grass blades from a block's `.ctr`
//! **density grid** ([`fileformats::pal5::ctr`]).
//!
//! ## Model (clean-room RE of `Pal5.exe`, see `generated/pal5_grass_re.md`)
//! A block's `.ctr` is a quadtree. Each leaf carries:
//! * a **per-cell density grid** over a rectangular sub-range of the block's
//!   coarse `16×16` grass grid (one cell per `320`-unit terrain patch), and
//! * optional **custom triangles** — which the RE showed are a
//!   **collision / pick** structure (the tall vertical "curtains"), **not** the
//!   visible grass. They are deliberately *not* rendered here; doing so was the
//!   source of the green-curtain / sky-cone artifacts.
//!
//! The **visible** grass is driven by the density grid: the original engine
//! generates upright grass blades on the flagged grid cells. We reproduce that
//! by collapsing every leaf's density grid into one per-block `16×16` coverage
//! map (max density per cell), then scattering crossed-quad grass tufts on the
//! covered cells, each draped on and standing up from the terrain surface
//! (heights bilinearly sampled from [`super::terrain::build_block_grass_heights`]).

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

/// Grass grid cells per block edge (one per `320`-unit terrain patch).
const GRID_CELLS: usize = 16;

/// Draw-distance for the grass chunk, in world units (tunable via
/// `PAL5_GRASS_DIST`).
const DEFAULT_DIST: f32 = 2400.0;

/// Grass tuft height in world units (tunable via `PAL5_GRASS_HEIGHT`). Blades
/// stand taller than they are wide so the field reads as upright grass rather
/// than a flat carpet.
const DEFAULT_HEIGHT: f32 = 42.0;

/// Grass tuft half-width in world units (tunable via `PAL5_GRASS_WIDTH`).
const DEFAULT_HALF_WIDTH: f32 = 16.0;

/// Multiplies the per-density tuft count (tunable via `PAL5_GRASS_DENSITY`).
/// Kept low so tufts stay sparse and the ground shows between them — a dense
/// `side×side` scatter reads as a solid green sheet.
const DEFAULT_DENSITY: f32 = 1.0;

/// Build the grass entity for one terrain block from its `.ctr` density grid.
/// Produces a single distance-culled [`StaticMeshComponent`] of crossed-quad
/// tufts grounded on `heights`. Returns an empty vec when the block has no
/// grass coverage.
pub fn build_block_grass(
    asset_loader: &AssetLoader,
    map_name: &str,
    block_tag: &str,
    heights: &BlockGrassHeights,
    leaves: &[GrassLeaf],
) -> Vec<ComRc<IEntity>> {
    let dist = env_f32("PAL5_GRASS_DIST", DEFAULT_DIST);
    let tuft_h = env_f32("PAL5_GRASS_HEIGHT", DEFAULT_HEIGHT);
    let half_w = env_f32("PAL5_GRASS_WIDTH", DEFAULT_HALF_WIDTH);
    let density_scale = env_f32("PAL5_GRASS_DENSITY", DEFAULT_DENSITY);

    // Collapse all leaves' density grids into one per-block coverage map: the
    // max density seen on each of the 16×16 cells. (Leaves are overlapping
    // texture layers over the same grid; we only need "is there grass here and
    // how dense".)
    let mut coverage = [[0u8; GRID_CELLS]; GRID_CELLS]; // [row][col]
    let mut any = false;
    for leaf in leaves {
        if leaf.density.is_empty() {
            continue;
        }
        for row in leaf.g[1]..=leaf.g[3] {
            for col in leaf.g[0]..=leaf.g[2] {
                if row < 0 || col < 0 || row as usize >= GRID_CELLS || col as usize >= GRID_CELLS {
                    continue;
                }
                if let Some(d) = leaf.density_at(col, row) {
                    if d > 0 {
                        let c = &mut coverage[row as usize][col as usize];
                        *c = (*c).max(d);
                        any = true;
                    }
                }
            }
        }
    }
    if !any {
        return Vec::new();
    }

    let material = SimpleMaterialDef::create_with_image("pal5_grass", Some(grass_texture()))
        .with_blend(BlendMode::AlphaTest)
        .with_cull(CullMode::None);
    let factory = asset_loader.component_factory();

    let max_idx = heights.corners.len().saturating_sub(1); // 16

    // Bilinear terrain height at world (x, z) inside the block grass grid. All
    // corners are finite (the heightfield is dilate-filled), so this never
    // returns NaN.
    let ground_at = |x: f32, z: f32| -> f32 {
        let fx = ((x - heights.min_x) / heights.cell_world).clamp(0.0, max_idx as f32);
        let fz = ((z - heights.min_z) / heights.cell_world).clamp(0.0, max_idx as f32);
        let r0 = fx.floor() as usize;
        let c0 = fz.floor() as usize;
        let r1 = (r0 + 1).min(max_idx);
        let c1 = (c0 + 1).min(max_idx);
        let tr = fx - r0 as f32;
        let tc = fz - c0 as f32;
        let h00 = heights.corners[r0][c0];
        let h10 = heights.corners[r1][c0];
        let h01 = heights.corners[r0][c1];
        let h11 = heights.corners[r1][c1];
        let h0 = h00 + (h10 - h00) * tr;
        let h1 = h01 + (h11 - h01) * tr;
        h0 + (h1 - h0) * tc
    };

    let mut vertices: Vec<Vec3> = Vec::new();
    let mut texcoords: Vec<TexCoord> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    for row in 0..GRID_CELLS {
        for col in 0..GRID_CELLS {
            let density = coverage[row][col];
            if density == 0 {
                continue;
            }

            // Only emit grass where the terrain under this cell is real. A cell's
            // four corners must all be backed by a decoded patch; dilate-filled
            // void corners carry extrapolated garbage heights (e.g. a block's
            // omitted region) and would float grass off into space — the source
            // of the green "strides across the scene".
            if !(heights.known[row][col]
                && heights.known[row + 1][col]
                && heights.known[row][col + 1]
                && heights.known[row + 1][col + 1])
            {
                continue;
            }

            // A handful of tufts per cell, scattered at random over the
            // `320`-unit cell. Count scales with density (observed 1/2/5/7).
            // Kept low: a dense sub-grid reads as an opaque sheet.
            let x0 = heights.min_x + row as f32 * heights.cell_world;
            let z0 = heights.min_z + col as f32 * heights.cell_world;
            let count = tuft_count(density, density_scale);
            for n in 0..count {
                // Deterministic jitter so the field is stable frame-to-frame
                // but not gridded.
                let seed = hash(row as u32, col as u32, n);
                let cx = x0 + frac(seed) * heights.cell_world;
                let cz = z0 + frac(seed.wrapping_mul(2654435761)) * heights.cell_world;
                let cy = ground_at(cx, cz);

                let hw = half_w * (0.7 + frac(seed.wrapping_mul(40503)) * 0.6);
                let hh = tuft_h * (0.7 + frac(seed.wrapping_mul(2246822519)) * 0.6);

                push_crossed_tuft(
                    &mut vertices,
                    &mut texcoords,
                    &mut indices,
                    &mut min,
                    &mut max,
                    cx,
                    cy,
                    cz,
                    hw,
                    hh,
                );
            }
        }
    }

    if indices.is_empty() {
        return Vec::new();
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

    // `SimpleMaterialDef` uses the `TexturedNoLight` shader, whose vertex layout
    // is exactly `POSITION | TEXCOORD`. Passing normals here would make the
    // buffer stride (which would then include NORMAL) disagree with the
    // pipeline's expected stride, shifting every vertex attribute and rendering
    // the mesh as garbage triangles radiating from a point. Omit normals.
    let geometry = Geometry::new(&vertices, None, &[texcoords], indices, material);

    let entity = CoreEntity::create(format!("{}_grass_{}", map_name, block_tag), false);
    let mesh = StaticMeshComponent::new(entity.clone(), vec![geometry], factory);
    entity.add_component(
        radiance::comdef::IStaticMeshComponent::uuid(),
        ComRc::from_object(mesh),
    );
    let cull = DistanceCullComponent::create(entity.clone(), center, dist + radius);
    entity.add_component(
        IDistanceCullComponent::uuid(),
        cull.query_interface::<IComponent>().unwrap(),
    );

    vec![entity]
}

/// Total tuft count for a density level over one `320`-unit cell. Density
/// values are small coverage levels (`1/2/5/7`); map them to just a few tufts so
/// the ground stays visible between them.
fn tuft_count(density: u8, scale: f32) -> u32 {
    let base = match density {
        0 => return 0,
        1 => 1.0,
        2 => 2.0,
        3 | 4 => 3.0,
        5 | 6 => 4.0,
        _ => 5.0,
    };
    ((base * scale).round() as u32).clamp(1, 16)
}

/// Append a crossed-billboard grass tuft (two perpendicular vertical quads)
/// standing on `(cx, cy, cz)` to the geometry buffers.
#[allow(clippy::too_many_arguments)]
fn push_crossed_tuft(
    vertices: &mut Vec<Vec3>,
    texcoords: &mut Vec<TexCoord>,
    indices: &mut Vec<u32>,
    min: &mut [f32; 3],
    max: &mut [f32; 3],
    cx: f32,
    cy: f32,
    cz: f32,
    half_w: f32,
    height: f32,
) {
    // Two quads at 90°: one spanning X, one spanning Z.
    let quads = [
        [(-half_w, 0.0), (half_w, 0.0)], // along X
        [(0.0, -half_w), (0.0, half_w)], // along Z
    ];
    for [(dx0, dz0), (dx1, dz1)] in quads {
        let base = vertices.len() as u32;
        // bottom-left, bottom-right, top-left, top-right (V=1 root, V=0 tip).
        let corners = [
            (cx + dx0, cy, cz + dz0, 0.0f32, 1.0f32),
            (cx + dx1, cy, cz + dz1, 1.0, 1.0),
            (cx + dx0, cy + height, cz + dz0, 0.0, 0.0),
            (cx + dx1, cy + height, cz + dz1, 1.0, 0.0),
        ];
        for (x, y, z, u, v) in corners {
            vertices.push(Vec3::new(x, y, z));
            texcoords.push(TexCoord::new(u, v));
            min[0] = min[0].min(x);
            min[1] = min[1].min(y);
            min[2] = min[2].min(z);
            max[0] = max[0].max(x);
            max[1] = max[1].max(y);
            max[2] = max[2].max(z);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }
}

fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a.wrapping_mul(73856093) ^ b.wrapping_mul(19349663) ^ c.wrapping_mul(83492791);
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^ (h >> 16)
}

/// Map a hashed `u32` to a `[0, 1)` float.
fn frac(h: u32) -> f32 {
    (h & 0x00ff_ffff) as f32 / 0x0100_0000 as f32
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v: &f32| v.is_finite() && *v > 0.0)
        .unwrap_or(default)
}

/// First-pass procedural grass-tuft texture: a sparse clump of tapering green
/// blades on a transparent background, so the alpha-test cutout reads as a tuft
/// and the bare ground shows between tufts. `V = 0` is the tip (top of image),
/// `V = 1` the root (bottom). Replaced by the authored grass atlas later.
fn grass_texture() -> RgbaImage {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut img = RgbaImage::from_pixel(W, H, Rgba([0, 0, 0, 0]));
    // A fan of a few near-upright blades rising from the bottom centre. Thin and
    // mostly transparent so overlapping tufts don't merge into a solid sheet.
    let blades: [(f32, f32, f32); 5] = [
        (0.32, 0.92, -0.05),
        (0.44, 0.80, 0.03),
        (0.50, 1.00, 0.00),
        (0.57, 0.84, -0.02),
        (0.68, 0.90, 0.06),
    ];
    for &(cx, height, lean) in &blades {
        let top = ((1.0 - height) * H as f32) as u32;
        for y in top..H {
            let t = (y - top) as f32 / (H - top).max(1) as f32; // 0 tip .. 1 root
            let half_w = 0.004 + 0.010 * t;
            // Muted olive-green, brighter near the root, so overlapping blades
            // read as soft grass instead of saturated lime streaks.
            let g = (96.0 + 56.0 * (1.0 - t)) as u8;
            let r = (44.0 + 36.0 * (1.0 - t)) as u8;
            let b = 30u8;
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
    use fileformats::pal5::ctr::GrassLeaf;

    fn flat_heights() -> BlockGrassHeights {
        BlockGrassHeights {
            min_x: 0.0,
            min_z: 0.0,
            cell_world: 320.0,
            corners: vec![[100.0; 17]; 17],
            known: vec![[true; 17]; 17],
        }
    }

    #[test]
    fn density_scales_tuft_count() {
        assert!(tuft_count(7, 1.0) > tuft_count(1, 1.0));
        assert_eq!(tuft_count(0, 1.0), 0);
        assert!(tuft_count(1, 1.0) >= 1);
    }

    /// Tufts stand up from the terrain surface: bottom on the ground, top a
    /// blade-height above — never anchored in the sky, and the coverage map
    /// only flags cells a leaf's density grid marks.
    #[test]
    fn coverage_and_tuft_geometry() {
        let leaf = GrassLeaf {
            tex0: 1,
            tex1: 5,
            g: [0, 0, 1, 1],
            density: vec![1, 2, 5, 7],
            vertices: vec![],
            triangles: vec![],
        };
        let heights = flat_heights();

        // Reproduce the coverage collapse + one cell's tuft emission.
        let mut coverage = [[0u8; GRID_CELLS]; GRID_CELLS];
        for row in leaf.g[1]..=leaf.g[3] {
            for col in leaf.g[0]..=leaf.g[2] {
                if let Some(d) = leaf.density_at(col, row) {
                    coverage[row as usize][col as usize] =
                        coverage[row as usize][col as usize].max(d);
                }
            }
        }
        // 2×2 cells all flagged.
        let flagged: usize = coverage
            .iter()
            .flatten()
            .filter(|&&d| d > 0)
            .count();
        assert_eq!(flagged, 4);

        // Build one tuft and confirm it rises from the ground (height 100).
        let mut v = vec![];
        let mut t = vec![];
        let mut idx = vec![];
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        push_crossed_tuft(
            &mut v, &mut t, &mut idx, &mut min, &mut max, 50.0,
            heights.corners[0][0], 50.0, 28.0, 45.0,
        );
        assert_eq!(v.len(), 8, "crossed tuft = 2 quads = 8 verts");
        assert_eq!(idx.len(), 12, "2 quads = 4 tris = 12 indices");
        assert!((min[1] - 100.0).abs() < 0.001, "roots on the ground");
        assert!((max[1] - 145.0).abs() < 0.001, "tips a blade-height above");
    }
}
