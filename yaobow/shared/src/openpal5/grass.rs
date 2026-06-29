//! PAL5 grass rendering.
//!
//! Renders the per-block grass authored in `<map>_<r>_<c>.ctr`
//! ([`fileformats::pal5::ctr`]) as **density-scaled 3D wind blades**, matching
//! the original engine's grass generator (`Pal5.exe 0x6d4510`).
//!
//! ## Model (clean-room RE of `Pal5.exe`, see `generated/pal5_grass_re.md`)
//! `.ctr` is the **sole, authoritative grass-distribution source** (verified
//! against the engine's own generator — `.nod` is objects, `.mp` is the terrain
//! + its texture splatting). A block's `.ctr` is a complete quadtree whose leaves
//! are **8×8 quadrants of a terrain patch**: leaf placement = `patch = tile/2`
//! plus the leaf's `g`-cell range (g0..g3 span 0..15 within the patch's 16×16
//! grid), mapping onto the terrain surface vertices. Each cell carries a
//! **density** byte (authored values 1/2/5/7); the density map is spatially
//! structured (clearings/verges dense, path corridor sparse).
//!
//! The engine reads each cell's density and emits **3D grass blades** scaled in
//! count and height by the density — there is **no flat grass-textured overlay**
//! (the ground appearance comes from the terrain `.mp` texture). The leaves'
//! in-file custom vertices/triangles are the engine's **collision / pick
//! "curtains"** and are not rendered.
//!
//! ## Rendering
//! For every density cell `>=` the blade threshold, crossed-quad grass-blade
//! billboards are scattered (count and height scale with the cell density), each
//! draped on the terrain heightfield and textured with a procedural green blade
//! `AlphaTest` cutout, sway-animated by the wind shader (roots pinned, tips
//! bend). Distance-culled per block. Sparse path-corridor cells (low density)
//! stay short/few or bare, so the dirt road reads through.
//!
//! A legacy flat ground overlay (`build_overlay_meshes`) remains behind
//! `PAL5_GRASS_OVERLAY=1` for debugging the density map, but is **off by
//! default** — it reads as "a layer on terrain" over the road.
//!
//! Tunables (env): `PAL5_GRASS_DIST`, `PAL5_GRASS_DENSITY_DIV`,
//! `PAL5_GRASS_BLADES`, `PAL5_GRASS_BLADE_MAX_DENSITY`,
//! `PAL5_GRASS_BLADES_PER_CELL`, `PAL5_GRASS_CELL_COVERAGE`,
//! `PAL5_GRASS_BLADE_HEIGHT`,
//! `PAL5_GRASS_BLADE_WIDTH`, `PAL5_GRASS_BLADE_TINT`, `PAL5_GRASS_BLADE_DIST`,
//! `PAL5_GRASS_WIND_STRENGTH`, `PAL5_GRASS_WIND_SPEED`, `PAL5_GRASS_OVERLAY`
//! (+ overlay-only: `PAL5_GRASS_UV_TILE`, `PAL5_GRASS_MIN_DENSITY`,
//! `PAL5_GRASS_TINT`, `PAL5_GRASS_ALPHA`, `PAL5_GRASS_LIFT`).

use std::cell::RefCell;
use std::collections::HashMap;

use crosscom::ComRc;
use fileformats::pal5::alp::terrain_texture_name;
use fileformats::pal5::ctr::GrassLeaf;
use image::{Rgba, RgbaImage};
use radiance::comdef::{IComponent, IDistanceCullComponent, IEntity};
use radiance::components::distance_cull::DistanceCullComponent;
use radiance::components::mesh::{Geometry, StaticMeshComponent, TexCoord};
use radiance::math::Vec3;
use radiance::rendering::{BlendMode, GrassMaterialDef};
use radiance::scene::CoreEntity;

use super::asset_loader::{AssetLoader, MapBlock};
use super::terrain::{BLOCK_WORLD_SIZE, BlockHeightField, block_height_field};

/// Draw-distance for a grass chunk, in world units (tunable via
/// `PAL5_GRASS_DIST`).
const DEFAULT_DIST: f32 = 3200.0;

/// World units per full repeat of the `cao###` grass texture (tunable via
/// `PAL5_GRASS_UV_TILE`).
const DEFAULT_UV_TILE: f32 = 160.0;

/// Density divisor: per-cell coverage = `min(density / div, 1)`. With the
/// authored values (1/2/5/7) `div = 5` maps `1 -> faint`, `5..7 -> full`.
/// Tunable via `PAL5_GRASS_DENSITY_DIV`.
const DEFAULT_DENSITY_DIV: f32 = 5.0;

/// Skip cells whose density is below this (tunable via `PAL5_GRASS_MIN_DENSITY`,
/// `1` = render every authored cell).
const DEFAULT_MIN_DENSITY: u32 = 1;

/// Grass colour multiplier (RGB). The `cao###` texture already supplies the
/// olive-green, so the default is neutral white; tunable via `PAL5_GRASS_TINT`
/// (`"r,g,b"`) to push the hue.
const DEFAULT_TINT: [f32; 3] = [1.0, 1.0, 1.0];

/// Overall coverage gain (the overlay's `tint.a`). Tunable via
/// `PAL5_GRASS_ALPHA`.
const DEFAULT_ALPHA: f32 = 0.9;

/// World units the overlay is lifted above the terrain to avoid z-fighting with
/// the coincident heightfield. Tunable via `PAL5_GRASS_LIFT`.
const DEFAULT_LIFT: f32 = 1.5;

/// Max horizontal sway in world units (tunable via `PAL5_GRASS_WIND_STRENGTH`).
/// Drives the upright blades; the flat overlay has wind weight 0 per vertex so
/// it never slides regardless.
const DEFAULT_WIND_STRENGTH: f32 = 3.0;

/// Wind oscillation rate, rad/sec (tunable via `PAL5_GRASS_WIND_SPEED`).
const DEFAULT_WIND_SPEED: f32 = 1.4;

/// Fallback grass texture id (`cao001`) when a leaf carries no valid `tex0`.
const FALLBACK_GRASS_TEX: i32 = 0;

/// Master switch for the upright grass blades (`PAL5_GRASS_BLADES`, `1`/`0`).
const DEFAULT_BLADES: bool = true;

/// Master switch for the legacy flat grass-ground overlay (`PAL5_GRASS_OVERLAY`).
/// **Off by default**: clean-room RE of `Pal5.exe` (grass generator `0x6d4510`)
/// shows the engine renders grass only as density-scaled 3D blades — there is no
/// flat overlay layer (the ground texture comes from the terrain `.mp`). The
/// overlay reads as "a layer on terrain" over paths, so it is disabled; enable
/// only to debug the density map.
const DEFAULT_OVERLAY: bool = false;

/// Cells with density `<=` this grow blades (tunable via
/// `PAL5_GRASS_BLADE_MAX_DENSITY`). **The `.ctr` density is the terrain
/// grass-texture blend, NOT grass amount**: clean-room RE + the gate reference
/// (`generated/pal5_grass.png`) prove density `1` is the grassy verge and the
/// higher values (2/5/7) are the bare/transition stone path. So grass grows
/// **only on the density-`1` verge** (`<= 1`); `2` is already path transition.
/// See `generated/pal5_grass_re.md`.
const DEFAULT_BLADE_MAX_DENSITY: u32 = 1;

/// Number of blade tufts per qualifying cell (tunable via
/// `PAL5_GRASS_BLADES_PER_CELL`).
const DEFAULT_BLADES_PER_CELL: f32 = 2.0;

/// Fraction of qualifying cells that actually grow grass, hashed per cell. Below
/// `1.0` this punches natural bare gaps so the verge reads as scattered clumps
/// (like the reference) instead of a solid carpet. Tunable via
/// `PAL5_GRASS_CELL_COVERAGE`.
const DEFAULT_CELL_COVERAGE: f32 = 0.78;

/// Blade tuft height in world units (tunable via `PAL5_GRASS_BLADE_HEIGHT`).
/// The reference grass is short, soft clumps — not tall reeds.
const DEFAULT_BLADE_HEIGHT: f32 = 10.0;

/// Blade tuft half-width in world units (tunable via `PAL5_GRASS_BLADE_WIDTH`).
const DEFAULT_BLADE_HALF_WIDTH: f32 = 7.0;

/// Blade colour multiplier (RGB); the procedural blade texture is near-white, so
/// this supplies the green. A muted olive (not vivid green) to match the
/// reference's soft, slightly-yellow verge grass. Tunable via
/// `PAL5_GRASS_BLADE_TINT` (`"r,g,b"`).
const DEFAULT_BLADE_TINT: [f32; 3] = [0.36, 0.50, 0.22];

/// Draw-distance for the blade layer (shorter than the overlay; tunable via
/// `PAL5_GRASS_BLADE_DIST`).
const DEFAULT_BLADE_DIST: f32 = 2200.0;

/// Shared `TextureStore` key for the procedural blade texture (built once).
const BLADE_TEX_NAME: &str = "pal5_grass_blade";

/// Build the grass-overlay entities for one terrain block from its `.ctr`
/// density grid (one mesh per `cao###` texture). Returns an empty vec when the
/// block carries no grass cells.
pub fn build_block_grass(
    asset_loader: &AssetLoader,
    map_name: &str,
    block: &MapBlock,
    leaves: &[GrassLeaf],
) -> Vec<ComRc<IEntity>> {
    let dist = env_f32("PAL5_GRASS_DIST", DEFAULT_DIST);
    let uv_tile = env_f32("PAL5_GRASS_UV_TILE", DEFAULT_UV_TILE).max(1.0);
    let density_div = env_f32("PAL5_GRASS_DENSITY_DIV", DEFAULT_DENSITY_DIV).max(1e-3);
    let min_density = env_u32("PAL5_GRASS_MIN_DENSITY", DEFAULT_MIN_DENSITY);
    let alpha = env_f32("PAL5_GRASS_ALPHA", DEFAULT_ALPHA).clamp(0.0, 1.0);
    let lift = env_f32("PAL5_GRASS_LIFT", DEFAULT_LIFT);
    let wind_strength = env_f32("PAL5_GRASS_WIND_STRENGTH", DEFAULT_WIND_STRENGTH);
    let wind_speed = env_f32("PAL5_GRASS_WIND_SPEED", DEFAULT_WIND_SPEED);
    let rgb = env_rgb("PAL5_GRASS_TINT").unwrap_or(DEFAULT_TINT);

    let heights = block_height_field(block);
    let factory = asset_loader.component_factory();
    let mut entities = Vec::new();

    // --- Flat grass-ground overlay (OFF by default) ---
    // Clean-room RE of `Pal5.exe` (grass generator `0x6d4510`) shows the engine
    // renders grass *only* as density-scaled 3D blades — there is no flat
    // grass-textured overlay layer; the ground appearance comes from the terrain
    // `.mp` texture. Our old overlay covered nearly every cell (incl. paths),
    // reading as "a layer on terrain" over the road. Kept behind a flag for
    // debugging the density map; enable with `PAL5_GRASS_OVERLAY=1`.
    if env_bool("PAL5_GRASS_OVERLAY", DEFAULT_OVERLAY) {
        let groups =
            build_overlay_meshes(leaves, &heights, uv_tile, density_div, min_density, lift);
        let mut tex_ids: Vec<i32> = groups.keys().copied().collect();
        tex_ids.sort_unstable();
        for tex_id in tex_ids {
            let Some(mesh) = groups[&tex_id].clone().finish() else {
                continue;
            };
            let tex_name = format!("pal5_grass_cao_{}", tex_id);
            let data = grass_texture_bytes(asset_loader, tex_id);
            let material = GrassMaterialDef::create_with_data(
                &tex_name,
                data,
                [rgb[0], rgb[1], rgb[2], alpha],
                wind_strength,
                wind_speed,
            );
            let geometry = Geometry::new(
                &mesh.vertices,
                None,
                &[mesh.uv_color, mesh.uv_wind],
                mesh.indices,
                material,
            );
            let entity = CoreEntity::create(
                format!("{}_grass_{}_{}_t{}", map_name, block.row, block.col, tex_id),
                false,
            );
            let sm = StaticMeshComponent::new(entity.clone(), vec![geometry], factory.clone());
            entity.add_component(
                radiance::comdef::IStaticMeshComponent::uuid(),
                ComRc::from_object(sm),
            );
            let cull =
                DistanceCullComponent::create(entity.clone(), mesh.center, dist + mesh.radius);
            entity.add_component(
                IDistanceCullComponent::uuid(),
                cull.query_interface::<IComponent>().unwrap(),
            );
            entities.push(entity);
        }
    }

    // --- Upright blade billboards over the denser cells ---
    let blades_on = env_bool("PAL5_GRASS_BLADES", DEFAULT_BLADES);
    if blades_on {
        let blade_max = env_u32("PAL5_GRASS_BLADE_MAX_DENSITY", DEFAULT_BLADE_MAX_DENSITY);
        let per_cell = env_f32("PAL5_GRASS_BLADES_PER_CELL", DEFAULT_BLADES_PER_CELL);
        let blade_h = env_f32("PAL5_GRASS_BLADE_HEIGHT", DEFAULT_BLADE_HEIGHT);
        let blade_w = env_f32("PAL5_GRASS_BLADE_WIDTH", DEFAULT_BLADE_HALF_WIDTH);
        let blade_dist = env_f32("PAL5_GRASS_BLADE_DIST", DEFAULT_BLADE_DIST);
        let cell_cov = env_f32("PAL5_GRASS_CELL_COVERAGE", DEFAULT_CELL_COVERAGE).clamp(0.0, 1.0);
        let blade_rgb = env_rgb("PAL5_GRASS_BLADE_TINT").unwrap_or(DEFAULT_BLADE_TINT);

        if let Some(mesh) = scatter_blades(
            leaves, &heights, blade_max, per_cell, cell_cov, blade_h, blade_w, lift,
        ) {
            let material = GrassMaterialDef::create_with_image(
                BLADE_TEX_NAME,
                Some(grass_blade_texture()),
                BlendMode::AlphaTest,
                [blade_rgb[0], blade_rgb[1], blade_rgb[2], 1.0],
                wind_strength,
                wind_speed,
            );
            let geometry = Geometry::new(
                &mesh.vertices,
                None,
                &[mesh.uv_color, mesh.uv_wind],
                mesh.indices,
                material,
            );
            let entity = CoreEntity::create(
                format!("{}_grassblade_{}_{}", map_name, block.row, block.col),
                false,
            );
            let sm = StaticMeshComponent::new(entity.clone(), vec![geometry], factory.clone());
            entity.add_component(
                radiance::comdef::IStaticMeshComponent::uuid(),
                ComRc::from_object(sm),
            );
            let cull = DistanceCullComponent::create(
                entity.clone(),
                mesh.center,
                blade_dist + mesh.radius,
            );
            entity.add_component(
                IDistanceCullComponent::uuid(),
                cull.query_interface::<IComponent>().unwrap(),
            );
            entities.push(entity);
        }
    }

    entities
}

/// Resolve a leaf's grass texture id: prefer `tex0`, then `tex1`, then the
/// [`FALLBACK_GRASS_TEX`].
fn grass_texture_id(leaf: &GrassLeaf) -> i32 {
    if leaf.tex0 >= 0 {
        leaf.tex0
    } else if leaf.tex1 >= 0 {
        leaf.tex1
    } else {
        FALLBACK_GRASS_TEX
    }
}

/// Emit one terrain-conformal quad per density cell, grouped by `cao###`
/// texture id. Empty-density leaves (collision-only) contribute nothing.
fn build_overlay_meshes(
    leaves: &[GrassLeaf],
    heights: &BlockHeightField,
    uv_tile: f32,
    density_div: f32,
    min_density: u32,
    lift: f32,
) -> HashMap<i32, MeshBuilder> {
    let (origin_x, origin_z) = heights.origin();
    let inv_tile = 1.0 / uv_tile;
    let mut groups: HashMap<i32, MeshBuilder> = HashMap::new();

    for leaf in leaves {
        if leaf.density.is_empty() || leaf.tiles_per_edge == 0 {
            continue;
        }
        let cols = leaf.cols().clamp(1, 64) as usize;
        let rows = leaf.rows().clamp(1, 64) as usize;
        if leaf.density.len() < cols * rows {
            continue;
        }
        let tile_world = BLOCK_WORLD_SIZE / leaf.tiles_per_edge as f32;
        let cell_w = tile_world / cols as f32;
        let cell_h = tile_world / rows as f32;
        let tile_x0 = origin_x + leaf.tile_x as f32 * tile_world;
        let tile_z0 = origin_z + leaf.tile_z as f32 * tile_world;

        let tex_id = grass_texture_id(leaf);
        let mb = groups.entry(tex_id).or_insert_with(MeshBuilder::new);

        for rr in 0..rows {
            for cc in 0..cols {
                let d = leaf.density[rr * cols + cc] as u32;
                if d < min_density {
                    continue;
                }
                let cov = (d as f32 / density_div).clamp(0.0, 1.0);
                let x0 = tile_x0 + cc as f32 * cell_w;
                let x1 = x0 + cell_w;
                let z0 = tile_z0 + rr as f32 * cell_h;
                let z1 = z0 + cell_h;
                push_cell_quad(mb, heights, x0, x1, z0, z1, inv_tile, cov, lift);
            }
        }
    }

    groups.retain(|_, mb| !mb.indices.is_empty());
    groups
}

/// Append one ground quad spanning `[x0,x1] × [z0,z1]`, each corner draped at
/// the terrain height (plus `lift`). `cov` is the per-cell coverage written to
/// the second texcoord set's `.y` (the shader scales alpha by it). Wind weight
/// (`.x`) is `0` — the overlay is flat ground and must not slide.
#[allow(clippy::too_many_arguments)]
fn push_cell_quad(
    mb: &mut MeshBuilder,
    heights: &BlockHeightField,
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    inv_tile: f32,
    cov: f32,
    lift: f32,
) {
    let corner = |mb: &mut MeshBuilder, x: f32, z: f32| -> u32 {
        let y = heights.sample(x, z) + lift;
        mb.push_vertex(x, y, z, x * inv_tile, z * inv_tile, 0.0, cov)
    };
    let v00 = corner(mb, x0, z0);
    let v10 = corner(mb, x1, z0);
    let v01 = corner(mb, x0, z1);
    let v11 = corner(mb, x1, z1);
    mb.indices
        .extend_from_slice(&[v00, v01, v10, v10, v01, v11]);
}

/// Scatter upright crossed-quad grass-blade billboards over the **low-density**
/// cells (`1 <= density <= blade_max`). The `.ctr` density is the terrain
/// grass-texture blend — low = grass-dominant verge/clearing, high = bare path —
/// so grass grows on the low cells (see `generated/pal5_grass_re.md`). Returns
/// `None` when no cell qualifies.
#[allow(clippy::too_many_arguments)]
fn scatter_blades(
    leaves: &[GrassLeaf],
    heights: &BlockHeightField,
    blade_max: u32,
    per_cell: f32,
    cell_cov: f32,
    blade_h: f32,
    half_w: f32,
    lift: f32,
) -> Option<Mesh> {
    let (origin_x, origin_z) = heights.origin();
    let mut mb = MeshBuilder::new();
    let mut salt: u32 = 0;

    for leaf in leaves {
        if leaf.density.is_empty() || leaf.tiles_per_edge == 0 {
            continue;
        }
        let cols = leaf.cols().clamp(1, 64) as usize;
        let rows = leaf.rows().clamp(1, 64) as usize;
        if leaf.density.len() < cols * rows {
            continue;
        }
        let tile_world = BLOCK_WORLD_SIZE / leaf.tiles_per_edge as f32;
        let cell_w = tile_world / cols as f32;
        let cell_h = tile_world / rows as f32;
        let tile_x0 = origin_x + leaf.tile_x as f32 * tile_world;
        let tile_z0 = origin_z + leaf.tile_z as f32 * tile_world;

        for rr in 0..rows {
            for cc in 0..cols {
                let d = leaf.density[rr * cols + cc] as u32;
                // Grass on grass-dominant (low-density) cells; skip the bare
                // path (high density) and uncovered (0) cells.
                if d < 1 || d > blade_max {
                    continue;
                }
                // Punch natural bare gaps: only a fraction of qualifying cells
                // grow grass (hashed by absolute cell position so it is stable),
                // so the verge reads as scattered clumps, not a solid carpet.
                let cell_key = hash(
                    (leaf.tile_x as u32) << 16 | leaf.tile_z as u32,
                    rr as u32,
                    cc as u32,
                );
                if frac(cell_key) >= cell_cov {
                    continue;
                }
                let count = (per_cell.round() as i32).clamp(1, 16);
                let x0 = tile_x0 + cc as f32 * cell_w;
                let z0 = tile_z0 + rr as f32 * cell_h;
                for n in 0..count as u32 {
                    salt = salt.wrapping_add(1);
                    let px = x0 + frac(hash(salt, n, 0x9e37)) * cell_w;
                    let pz = z0 + frac(hash(salt, n, 0x85eb)) * cell_h;
                    let py = heights.sample(px, pz) + lift;
                    let hw = half_w * (0.7 + frac(hash(salt, n, 0x27d4)) * 0.6);
                    let hh = blade_h * (0.7 + frac(hash(salt, n, 0x1656)) * 0.6);
                    push_crossed_blade(&mut mb, px, py, pz, hw, hh);
                }
            }
        }
    }

    mb.finish()
}

/// Append a crossed-billboard grass tuft (two perpendicular vertical quads)
/// rooted at `(cx, cy, cz)`. `V = 1` at the root, `V = 0` at the tip; the wind
/// tip weight is `1 - V` (roots pinned, tips bend). Coverage is `1` (the
/// `AlphaTest` cutout is the silhouette).
fn push_crossed_blade(mb: &mut MeshBuilder, cx: f32, cy: f32, cz: f32, half_w: f32, height: f32) {
    let quads = [
        [(-half_w, 0.0), (half_w, 0.0)], // along X
        [(0.0, -half_w), (0.0, half_w)], // along Z
    ];
    for [(dx0, dz0), (dx1, dz1)] in quads {
        let bl = mb.push_vertex(cx + dx0, cy, cz + dz0, 0.0, 1.0, 0.0, 1.0);
        let br = mb.push_vertex(cx + dx1, cy, cz + dz1, 1.0, 1.0, 0.0, 1.0);
        let tl = mb.push_vertex(cx + dx0, cy + height, cz + dz0, 0.0, 0.0, 1.0, 1.0);
        let tr = mb.push_vertex(cx + dx1, cy + height, cz + dz1, 1.0, 0.0, 1.0, 1.0);
        mb.indices.extend_from_slice(&[bl, br, tl, tl, br, tr]);
    }
}

/// Procedural grass-blade texture: a bushy clump of tapering near-white blades
/// of varied height on a transparent background, so the `AlphaTest` cutout reads
/// as a clump of short grass. The blade colour is near-white (the material tint
/// supplies the green). `V = 0` is the tip (top), `V = 1` the root (bottom).
fn grass_blade_texture() -> RgbaImage {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut img = RgbaImage::from_pixel(W, H, Rgba([0, 0, 0, 0]));
    let blades: [(f32, f32, f32); 13] = [
        (0.10, 0.45, -0.04),
        (0.18, 0.60, 0.03),
        (0.24, 0.38, -0.02),
        (0.32, 0.72, 0.05),
        (0.38, 0.50, -0.03),
        (0.45, 0.85, 0.01),
        (0.50, 0.55, -0.01),
        (0.56, 0.95, 0.02),
        (0.62, 0.48, 0.04),
        (0.70, 0.70, -0.05),
        (0.76, 0.42, 0.03),
        (0.84, 0.62, -0.02),
        (0.92, 0.40, 0.05),
    ];
    for &(cx, height, lean) in &blades {
        let top = ((1.0 - height) * H as f32) as u32;
        for y in top..H {
            let t = (y - top) as f32 / (H - top).max(1) as f32; // 0 tip .. 1 root
            let half_w = 0.006 + 0.013 * t;
            let l = (170.0 + 70.0 * (1.0 - t)) as u8;
            let lx = cx + lean * (1.0 - t);
            let x0 = ((lx - half_w) * W as f32).floor().max(0.0) as u32;
            let x1 = ((lx + half_w) * W as f32).ceil().min(W as f32) as u32;
            for x in x0..x1 {
                img.put_pixel(x, y, Rgba([l, l, l, 255]));
            }
        }
    }
    img
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

/// Accumulated overlay geometry (position + colour UV + coverage UV) + bounds.
struct Mesh {
    vertices: Vec<Vec3>,
    uv_color: Vec<TexCoord>,
    /// `.x` = wind weight (0 = pinned, flat overlay), `.y` = per-cell coverage.
    uv_wind: Vec<TexCoord>,
    indices: Vec<u32>,
    center: Vec3,
    radius: f32,
}

#[derive(Clone)]
struct MeshBuilder {
    vertices: Vec<Vec3>,
    uv_color: Vec<TexCoord>,
    uv_wind: Vec<TexCoord>,
    indices: Vec<u32>,
    min: [f32; 3],
    max: [f32; 3],
}

impl MeshBuilder {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            uv_color: Vec::new(),
            uv_wind: Vec::new(),
            indices: Vec::new(),
            min: [f32::MAX; 3],
            max: [f32::MIN; 3],
        }
    }

    fn push_vertex(&mut self, x: f32, y: f32, z: f32, u: f32, v: f32, wind: f32, cov: f32) -> u32 {
        let idx = self.vertices.len() as u32;
        self.vertices.push(Vec3::new(x, y, z));
        self.uv_color.push(TexCoord::new(u, v));
        self.uv_wind.push(TexCoord::new(wind, cov));
        self.min[0] = self.min[0].min(x);
        self.min[1] = self.min[1].min(y);
        self.min[2] = self.min[2].min(z);
        self.max[0] = self.max[0].max(x);
        self.max[1] = self.max[1].max(y);
        self.max[2] = self.max[2].max(z);
        idx
    }

    fn finish(self) -> Option<Mesh> {
        if self.indices.is_empty() {
            return None;
        }
        let center = Vec3::new(
            0.5 * (self.min[0] + self.max[0]),
            0.5 * (self.min[1] + self.max[1]),
            0.5 * (self.min[2] + self.max[2]),
        );
        let radius = 0.5
            * ((self.max[0] - self.min[0]).powi(2) + (self.max[2] - self.min[2]).powi(2))
                .sqrt()
                .max(1.0);
        Some(Mesh {
            vertices: self.vertices,
            uv_color: self.uv_color,
            uv_wind: self.uv_wind,
            indices: self.indices,
            center,
            radius,
        })
    }
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v: &f32| v.is_finite() && *v >= 0.0)
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(s) => !matches!(s.trim(), "0" | "false" | "off" | "no" | ""),
        Err(_) => default,
    }
}

/// Parse a `"r,g,b"` env var (each `0..=1`) into an RGB tint override.
fn env_rgb(key: &str) -> Option<[f32; 3]> {
    let s = std::env::var(key).ok()?;
    let parts: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if parts.len() == 3 && parts.iter().all(|v| v.is_finite()) {
        Some([parts[0], parts[1], parts[2]])
    } else {
        None
    }
}

thread_local! {
    /// Per-texture-id cache of raw `cao###` DDS bytes (read from the vfs once
    /// per scene build; radiance's `TextureStore` then decodes the DXT5 once).
    static TEX_CACHE: RefCell<HashMap<i32, Option<Vec<u8>>>> = RefCell::new(HashMap::new());
}

/// Raw `cao###` grass DDS bytes for a texture id (cached per id). Decoded by
/// radiance (DXT5) — `image` 0.23 in `shared` cannot decode DDS, so the bytes
/// are routed raw. `None` when the id/file is unavailable.
fn grass_texture_bytes(asset_loader: &AssetLoader, tex_id: i32) -> Option<Vec<u8>> {
    TEX_CACHE.with(|cache| {
        if let Some(bytes) = cache.borrow().get(&tex_id) {
            return bytes.clone();
        }
        let bytes = load_cao_bytes(asset_loader, tex_id);
        cache.borrow_mut().insert(tex_id, bytes.clone());
        bytes
    })
}

/// Read raw `cao###` DDS bytes by id from the `/Texture/TerrainTexture/` path
/// the terrain splat uses.
fn load_cao_bytes(asset_loader: &AssetLoader, tex_id: i32) -> Option<Vec<u8>> {
    if tex_id < 0 {
        return None;
    }
    let name = terrain_texture_name(tex_id as u8)?;
    let path = format!("/Texture/TerrainTexture/{}", name);
    asset_loader.read_file(&path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fileformats::pal5::ctr::{GrassLeaf, GrassTri};

    /// A flat block heightfield at a constant height, origin (0,0).
    fn flat_heights() -> BlockHeightField {
        // 2x2 grid is enough; sample clamps. Build via the real constructor is
        // not possible (private fields), so fabricate through the public path
        // is unnecessary — tests only need `sample`, exercised via build below.
        BlockHeightField::flat_for_test(0.0, 0.0, 100.0)
    }

    /// A density-grid leaf at quadtree tile `(tx,tz)` of a depth-5 tree (32
    /// tiles/edge), with an 8×8 density block.
    fn density_leaf(tex0: i32, tx: u32, tz: u32, density: Vec<u8>) -> GrassLeaf {
        GrassLeaf {
            tex0,
            tex1: -1,
            g: [0, 0, 7, 7], // 8×8
            density,
            tile_x: tx,
            tile_z: tz,
            tiles_per_edge: 32,
            vertices: vec![],
            triangles: vec![],
        }
    }

    #[test]
    fn empty_density_leaf_yields_no_grass() {
        // A collision-only leaf (custom verts/tris, no density) renders nothing.
        let collision = GrassLeaf {
            tex0: 11,
            tex1: -1,
            g: [0, 0, 0, 0],
            density: vec![],
            tile_x: 0,
            tile_z: 0,
            tiles_per_edge: 32,
            vertices: vec![[0.0, 100.0, 0.0], [60.0, 100.0, 0.0], [0.0, 700.0, 0.0]],
            triangles: vec![GrassTri {
                indices: [0, 1, 2],
                color: 0x00ff_0002,
            }],
        };
        let groups = build_overlay_meshes(&[collision], &flat_heights(), 160.0, 5.0, 1, 1.0);
        assert!(groups.is_empty(), "collision-only leaf produces no grass");
    }

    #[test]
    fn density_cells_become_quads_grouped_by_texture() {
        let leaf = density_leaf(11, 0, 0, vec![5u8; 64]);
        let groups = build_overlay_meshes(&[leaf], &flat_heights(), 160.0, 5.0, 1, 1.0);
        assert_eq!(groups.len(), 1, "one cao texture group");
        let mb = &groups[&11];
        // 64 cells × (4 verts, 6 indices).
        assert_eq!(mb.vertices.len(), 64 * 4);
        assert_eq!(mb.indices.len(), 64 * 6);
    }

    #[test]
    fn min_density_threshold_drops_sparse_cells() {
        // Half the cells at density 1, half at 7; threshold 2 keeps only 7s.
        let mut d = vec![1u8; 64];
        for v in d.iter_mut().take(32) {
            *v = 7;
        }
        let leaf = density_leaf(11, 0, 0, d);
        let all = build_overlay_meshes(&[leaf.clone()], &flat_heights(), 160.0, 5.0, 1, 1.0);
        let some = build_overlay_meshes(&[leaf], &flat_heights(), 160.0, 5.0, 2, 1.0);
        assert_eq!(all[&11].vertices.len(), 64 * 4);
        assert_eq!(some[&11].vertices.len(), 32 * 4, "only the dense cells");
    }

    #[test]
    fn coverage_scales_with_density() {
        let sparse = density_leaf(11, 0, 0, vec![1u8; 64]);
        let dense = density_leaf(11, 0, 0, vec![7u8; 64]);
        let gs = build_overlay_meshes(&[sparse], &flat_heights(), 160.0, 5.0, 1, 1.0);
        let gd = build_overlay_meshes(&[dense], &flat_heights(), 160.0, 5.0, 1, 1.0);
        let cov_s = gs[&11].uv_wind[0].v;
        let cov_d = gd[&11].uv_wind[0].v;
        assert!(cov_d > cov_s, "denser cell -> higher coverage");
        assert!((cov_d - 1.0).abs() < 1e-4, "density 7 / div 5 clamps to 1");
    }

    #[test]
    fn tile_position_places_cells_in_world() {
        // Tile (1,0) of a 32-tile block (5120u) starts at x = 160, z = 0.
        let leaf = density_leaf(11, 1, 0, vec![5u8; 64]);
        let groups = build_overlay_meshes(&[leaf], &flat_heights(), 160.0, 5.0, 1, 0.0);
        let mb = &groups[&11];
        let minx = mb.vertices.iter().map(|v| v.x).fold(f32::MAX, f32::min);
        let maxx = mb.vertices.iter().map(|v| v.x).fold(f32::MIN, f32::max);
        let minz = mb.vertices.iter().map(|v| v.z).fold(f32::MAX, f32::min);
        assert!((minx - 160.0).abs() < 1e-3, "tile_x=1 -> x starts at 160");
        assert!((maxx - 320.0).abs() < 1e-3, "tile spans 160 units");
        assert!(minz.abs() < 1e-3, "tile_z=0 -> z starts at 0");
        // Draped on the flat 100u terrain (lift 0).
        assert!((mb.vertices[0].y - 100.0).abs() < 1e-3);
    }

    #[test]
    fn blades_scatter_only_on_low_density_grass_cells() {
        // Half the cells low-density grass (1), half high-density path (7);
        // blade_max = 2 -> only the 32 grass cells grow blades.
        let mut d = vec![7u8; 64];
        for v in d.iter_mut().take(32) {
            *v = 1;
        }
        let leaf = density_leaf(11, 0, 0, d);
        let mesh = scatter_blades(&[leaf], &flat_heights(), 2, 2.0, 1.0, 26.0, 11.0, 0.0)
            .expect("grass cells emit blades");
        // Crossed tuft = 2 quads = 8 verts, 12 indices.
        assert_eq!(mesh.vertices.len() % 8, 0);
        assert_eq!(mesh.indices.len(), mesh.vertices.len() / 8 * 12);
        // Roots sit on the flat 100u terrain; tips rise above it.
        let ymin = mesh.vertices.iter().map(|v| v.y).fold(f32::MAX, f32::min);
        let ymax = mesh.vertices.iter().map(|v| v.y).fold(f32::MIN, f32::max);
        assert!((ymin - 100.0).abs() < 1e-3, "roots on the ground");
        assert!(ymax > 100.0 + 15.0, "tips rise above the ground");
    }

    #[test]
    fn no_blades_when_all_cells_are_high_density_path() {
        let leaf = density_leaf(11, 0, 0, vec![7u8; 64]);
        let mesh = scatter_blades(&[leaf], &flat_heights(), 2, 2.0, 1.0, 26.0, 11.0, 0.0);
        assert!(
            mesh.is_none(),
            "all cells are path (density 7 > blade_max=2)"
        );
    }
}
