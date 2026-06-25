//! PAL5 grass rendering.
//!
//! Builds a renderable entity from the grass data decoded out of a map's
//! `.ctr` blocks ([`fileformats::pal5::ctr`]).
//!
//! ## What the `.ctr` actually stores
//! Each `.ctr` leaf is **not** a set of grass blades — it is the small
//! ground-surface patch (world-space vertices + triangle mesh, plus a packed
//! density map) that the original engine *scatters* grass blades onto. The
//! shipped game generates a separate 44-byte-per-vertex blade buffer
//! (`grassVertexData`, `Pal5.exe 0x6d4900`) from this surface; drawing the
//! surface triangles directly would just paint the terrain solid green.
//!
//! ## What we do
//! We reproduce the *effect* rather than the exact scatter: a small upright
//! **cross-billboard** (two perpendicular quads with a per-blade yaw/height
//! jitter, so it reads as grass from any view angle) is planted at every Nth
//! surface vertex — i.e. on the ground, at the authored anchor positions.
//! Blade size/density are tunable via `PAL5_GRASS_HEIGHT` /
//! `PAL5_GRASS_WIDTH` / `PAL5_GRASS_STRIDE`.
//!
//! Each `.ctr` quadtree leaf becomes one [`StaticMeshComponent`] chunk
//! entity carrying a [`DistanceCullComponent`], reproducing the engine's
//! `GrassDist` draw-distance cull (tunable via `PAL5_GRASS_DIST`): only
//! chunks near the camera are drawn, so the field fades to bare ground in
//! the distance instead of stacking into an opaque green wall to the horizon.
//!
//! Resolving the authored grass textures (`tex0`/`tex1` -> `Texture\grass\`),
//! the per-cell density map, and wind animation are later refinements.

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

/// Default grass-blade height in world units (tunable via `PAL5_GRASS_HEIGHT`).
/// A terrain cell is 320 units, so ~0.13 cell tall reads as a knee-high tuft
/// from the ~35-deg-down gameplay camera while staying short enough that it
/// does not wall up the view.
const DEFAULT_HEIGHT: f32 = 42.0;
/// Default grass-blade width in world units (tunable via `PAL5_GRASS_WIDTH`).
const DEFAULT_WIDTH: f32 = 34.0;
/// Plant a blade at every Nth anchor (tunable via `PAL5_GRASS_STRIDE`). The
/// `.ctr` surface vertices are denser than playable grass needs; subsampling
/// keeps the field readable (ground visible between tufts). `1` = every anchor.
const DEFAULT_STRIDE: usize = 3;
/// Draw-distance for a grass chunk, in world units (tunable via
/// `PAL5_GRASS_DIST`). A chunk is only drawn while the camera is within this
/// distance of the chunk (plus the chunk's own radius) — the engine's
/// `GrassDist`. Without it the upright cards stack into an opaque green wall
/// to the horizon; with it the field fades to bare ground in the distance.
const DEFAULT_DIST: f32 = 1600.0;

/// Guard against a pathological/corrupt block flooding the GPU buffer.
const MAX_BLADES: usize = 400_000;

/// Build the grass chunk entities for `map_name` from its decoded `.ctr`
/// leaves. Each `.ctr` leaf (a spatial quadtree cell) becomes one
/// distance-culled [`StaticMeshComponent`] entity, so only chunks near the
/// camera are drawn. Returns an empty vec when the map ships no grass.
pub fn build_grass_entities(
    asset_loader: &AssetLoader,
    map_name: &str,
    leaves: &[GrassLeaf],
) -> Vec<ComRc<IEntity>> {
    if leaves.is_empty() {
        return Vec::new();
    }

    let height = env_f32("PAL5_GRASS_HEIGHT", DEFAULT_HEIGHT);
    let width = env_f32("PAL5_GRASS_WIDTH", DEFAULT_WIDTH);
    let stride = env_usize("PAL5_GRASS_STRIDE", DEFAULT_STRIDE).max(1);
    let dist = env_f32("PAL5_GRASS_DIST", DEFAULT_DIST);
    let half = width * 0.5;

    // Share one cutout grass material across every chunk.
    let material = SimpleMaterialDef::create_with_image("pal5_grass_blade", Some(grass_texture()))
        .with_blend(BlendMode::AlphaTest)
        .with_cull(CullMode::None);
    let factory = asset_loader.component_factory();

    let mut entities: Vec<ComRc<IEntity>> = Vec::new();
    let mut total_blades = 0usize;
    // Subsample across the whole field (not per chunk) so density is uniform
    // and small chunks still get blades.
    let mut anchor_index = 0usize;

    for (leaf_idx, leaf) in leaves.iter().enumerate() {
        let mut vertices: Vec<Vec3> = Vec::new();
        let mut normals: Vec<Vec3> = Vec::new();
        let mut texcoords: Vec<TexCoord> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        // Chunk bounds (for the cull centre + radius).
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];

        for anchor in &leaf.vertices {
            let take = anchor_index % stride == 0;
            anchor_index += 1;
            if !take {
                continue;
            }
            let [ax, ay, az] = *anchor;
            if !(ax.is_finite() && ay.is_finite() && az.is_finite()) {
                continue;
            }
            for i in 0..3 {
                min[i] = min[i].min(anchor[i]);
                max[i] = max[i].max(anchor[i]);
            }
            // Deterministic per-blade jitter (from the anchor position) so the
            // tufts read as scattered grass instead of an axis-aligned grid:
            // a random yaw turns the cross so some blade faces every camera,
            // and a height variation breaks the uniform skyline.
            let h = hash_pos(ax, ay, az);
            let yaw = (h & 0xffff) as f32 / 65535.0 * std::f32::consts::TAU;
            let hjit = 0.7 + ((h >> 16) & 0xff) as f32 / 255.0 * 0.6; // 0.7..1.3
            let blade_h = height * hjit;
            let (s, c) = yaw.sin_cos();
            let (dx0, dz0) = (c * half, s * half);
            let (dx1, dz1) = (-s * half, c * half);
            let top = ay + blade_h;
            push_quad(
                &mut vertices,
                &mut normals,
                &mut texcoords,
                &mut indices,
                [ax - dx0, ay, az - dz0],
                [ax + dx0, ay, az + dz0],
                top,
            );
            push_quad(
                &mut vertices,
                &mut normals,
                &mut texcoords,
                &mut indices,
                [ax - dx1, ay, az - dz1],
                [ax + dx1, ay, az + dz1],
                top,
            );

            total_blades += 1;
            if total_blades >= MAX_BLADES {
                break;
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
        // Cull radius covers the chunk's own footprint plus the draw distance,
        // so a chunk the player is standing inside never pops out.
        let chunk_radius = 0.5
            * ((max[0] - min[0]).powi(2) + (max[2] - min[2]).powi(2))
                .sqrt()
                .max(0.0);

        let geometry = Geometry::new(
            &vertices,
            Some(&normals),
            &[texcoords],
            indices,
            material.clone(),
        );

        // Start hidden; the cull component shows the chunk when the camera is
        // near (and the engine fires `on_loading`/`on_updating` from frame 1).
        let entity = CoreEntity::create(format!("{}_grass_{}", map_name, leaf_idx), false);
        let mesh = StaticMeshComponent::new(entity.clone(), vec![geometry], factory.clone());
        entity.add_component(
            radiance::comdef::IStaticMeshComponent::uuid(),
            ComRc::from_object(mesh),
        );
        let cull = DistanceCullComponent::create(entity.clone(), center, dist + chunk_radius);
        entity.add_component(
            IDistanceCullComponent::uuid(),
            cull.query_interface::<IComponent>().unwrap(),
        );
        entities.push(entity);

        if total_blades >= MAX_BLADES {
            break;
        }
    }

    log::info!(
        "Pal5 grass '{}': {} chunks, {} blades (dist {})",
        map_name,
        entities.len(),
        total_blades,
        dist,
    );
    entities
}

/// Append one upright quad: base edge `base0..base1` (at their own `y`) rising
/// to `top_y`, as two triangles. UVs map the full grass texture.
fn push_quad(
    vertices: &mut Vec<Vec3>,
    normals: &mut Vec<Vec3>,
    texcoords: &mut Vec<TexCoord>,
    indices: &mut Vec<u32>,
    base0: [f32; 3],
    base1: [f32; 3],
    top_y: f32,
) {
    let base = vertices.len() as u32;
    // 0: base-left, 1: base-right, 2: top-right, 3: top-left.
    vertices.push(Vec3::new(base0[0], base0[1], base0[2]));
    vertices.push(Vec3::new(base1[0], base1[1], base1[2]));
    vertices.push(Vec3::new(base1[0], top_y, base1[2]));
    vertices.push(Vec3::new(base0[0], top_y, base0[2]));
    for _ in 0..4 {
        normals.push(Vec3::new(0.0, 1.0, 0.0));
    }
    // v=1 at the base (texture bottom), v=0 at the tip (texture top).
    texcoords.push(TexCoord::new(0.0, 1.0));
    texcoords.push(TexCoord::new(1.0, 1.0));
    texcoords.push(TexCoord::new(1.0, 0.0));
    texcoords.push(TexCoord::new(0.0, 0.0));
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Cheap deterministic hash of a world position, used for per-blade jitter so
/// the grass field looks scattered rather than a regular grid. Stable across
/// runs (no RNG state) so the field is identical frame-to-frame.
fn hash_pos(x: f32, y: f32, z: f32) -> u32 {
    let mut h = 2166136261u32;
    for v in [x, y, z] {
        h ^= v.to_bits();
        h = h.wrapping_mul(16777619);
        h ^= h >> 13;
    }
    h
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v: &f32| v.is_finite() && *v > 0.0)
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// First-pass procedural grass-blade texture: a few green blades on a
/// transparent ground so the cutout material trims the gaps and the blade
/// silhouette reads as grass (not a solid green card). Cached by name.
fn grass_texture() -> RgbaImage {
    const W: u32 = 32;
    const H: u32 = 32;
    let transparent = Rgba([0u8, 0, 0, 0]);
    let mut img = RgbaImage::from_pixel(W, H, transparent);

    // Several tapering blades fanning from a common root, so the cutout
    // silhouette reads as a soft grass tuft rather than a few fat bars.
    let blades: [(f32, f32); 6] = [
        (0.20, 0.10),
        (0.34, -0.06),
        (0.46, 0.04),
        (0.56, -0.10),
        (0.68, 0.08),
        (0.80, -0.04),
    ];
    for y in 0..H {
        // 0 = texture top (tip), H-1 = bottom (root).
        let t = y as f32 / (H - 1) as f32; // 0 tip .. 1 root
                                           // brighter, yellower toward the tip; darker green at the root.
        let g = (105.0 + 125.0 * (1.0 - t)) as u8;
        let r = (35.0 + 75.0 * (1.0 - t)) as u8;
        let b = 26u8;
        // Blade half-width grows toward the root (taper to a point at the tip).
        let half_w = 0.010 + 0.030 * t;
        for &(cx, lean) in &blades {
            let center = cx + lean * (1.0 - t); // lean toward the tip
            let x0 = ((center - half_w) * W as f32).floor().max(0.0) as u32;
            let x1 = ((center + half_w) * W as f32).ceil().min(W as f32) as u32;
            for x in x0..x1 {
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
    }
    img
}
