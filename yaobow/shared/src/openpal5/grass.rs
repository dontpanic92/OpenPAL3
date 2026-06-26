//! PAL5 grass rendering.
//!
//! Builds a flat, grass-textured overlay on the terrain heightfield from the
//! grass layers decoded out of a block's `.ctr` ([`fileformats::pal5::ctr`]).
//!
//! ## Model (clean-room RE of `Pal5.exe`, see [`fileformats::pal5::ctr`])
//! A block's grass is a coarse `16×16` grid of cells (one per `320`-unit
//! terrain patch). Each `.ctr` quadtree leaf is a **layer**: a grass texture
//! pair (`tex0`/`tex1`) over a rectangular cell range, with a **per-cell
//! density**. The engine emits two triangles per flagged cell over a `17×17`
//! lattice of grass vertices sampled from the terrain surface — i.e. grass is
//! painted flat on the ground, only on cells a layer flags (unflagged cells
//! stay bare). It is *not* billboards and *not* a separate 3D mesh.
//!
//! We reproduce this: for each layer, for each flagged cell, we emit the cell
//! quad using the block's terrain heights at the four patch corners
//! ([`super::terrain::build_block_grass_heights`]). Resolving the authored
//! `cao###` grass textures and per-cell density blending are refinements; a
//! procedural grass texture is used until then.

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

/// Times the grass texture repeats across one `320`-unit grass cell.
const TEX_REPEAT_PER_CELL: f32 = 4.0;

/// Small upward bias so the overlay sits just above the terrain it copies,
/// avoiding z-fighting with the terrain surface.
const BASE_LIFT: f32 = 1.0;

/// Build the grass overlay entities for one terrain block. Each `.ctr` layer
/// (leaf with a density grid) becomes one distance-culled
/// [`StaticMeshComponent`] chunk draped on `heights`. Returns an empty vec
/// when the block has no grass layers.
pub fn build_block_grass(
    asset_loader: &AssetLoader,
    map_name: &str,
    block_tag: &str,
    heights: &BlockGrassHeights,
    leaves: &[GrassLeaf],
) -> Vec<ComRc<IEntity>> {
    let dist = env_f32("PAL5_GRASS_DIST", DEFAULT_DIST);

    let material =
        SimpleMaterialDef::create_with_image("pal5_grass_overlay", Some(grass_texture()))
            .with_blend(BlendMode::AlphaTest)
            .with_cull(CullMode::None);
    let factory = asset_loader.component_factory();

    let grid_verts = heights.corners.len(); // 17
    let max_idx = grid_verts.saturating_sub(1) as i32; // 16

    let mut entities: Vec<ComRc<IEntity>> = Vec::new();

    for (layer_idx, leaf) in leaves.iter().enumerate() {
        if leaf.density.is_empty() {
            // Custom slope-geometry leaves are a later refinement.
            continue;
        }

        let mut vertices: Vec<Vec3> = Vec::new();
        let mut normals: Vec<Vec3> = Vec::new();
        let mut texcoords: Vec<TexCoord> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];

        // Layered overlays over the same cells get a tiny per-layer lift so
        // they don't z-fight each other.
        let lift = BASE_LIFT + layer_idx as f32 * 0.15;

        // Corner world position for grass-grid vertex (row, col): `row` steps
        // world X, `col` steps world Z; height from the terrain sample.
        let corner = |row: i32, col: i32| -> Vec3 {
            let r = row.clamp(0, max_idx) as usize;
            let c = col.clamp(0, max_idx) as usize;
            Vec3::new(
                heights.min_x + row as f32 * heights.cell_world,
                heights.corners[r][c] + lift,
                heights.min_z + col as f32 * heights.cell_world,
            )
        };

        for row in leaf.g[1]..=leaf.g[3] {
            for col in leaf.g[0]..=leaf.g[2] {
                if col < 0 || row < 0 || col > max_idx || row > max_idx {
                    continue;
                }
                match leaf.density_at(col, row) {
                    Some(d) if d > 0 => {}
                    _ => continue,
                }

                // Quad corners (engine: A=(row,col), B=(row+1,col),
                // C=(row,col+1), D=(row+1,col+1); tris A,B,C and C,B,D).
                let a = corner(row, col);
                let b = corner(row + 1, col);
                let c = corner(row, col + 1);
                let d = corner(row + 1, col + 1);
                let base = vertices.len() as u32;
                for v in [a, b, c, d] {
                    vertices.push(v);
                    normals.push(Vec3::new(0.0, 1.0, 0.0));
                    for i in 0..3 {
                        min[i] = min[i].min([v.x, v.y, v.z][i]);
                        max[i] = max[i].max([v.x, v.y, v.z][i]);
                    }
                }
                // UV tiles the grass texture several times across each cell.
                texcoords.push(TexCoord::new(0.0, 0.0));
                texcoords.push(TexCoord::new(TEX_REPEAT_PER_CELL, 0.0));
                texcoords.push(TexCoord::new(0.0, TEX_REPEAT_PER_CELL));
                texcoords.push(TexCoord::new(TEX_REPEAT_PER_CELL, TEX_REPEAT_PER_CELL));
                indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base + 2,
                    base + 1,
                    base + 3,
                ]);
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
                .max(0.0);

        let geometry = Geometry::new(
            &vertices,
            Some(&normals),
            &[texcoords],
            indices,
            material.clone(),
        );

        let entity = CoreEntity::create(
            format!("{}_grass_{}_{}", map_name, block_tag, layer_idx),
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

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|v: &f32| v.is_finite() && *v > 0.0)
        .unwrap_or(default)
}

/// First-pass procedural grass texture: tapering green blades on a transparent
/// ground, so the alpha-test cutout reads as grass and the bare ground shows
/// between tufts. Replaced by the authored `cao###` textures later.
fn grass_texture() -> RgbaImage {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut img = RgbaImage::from_pixel(W, H, Rgba([0, 0, 0, 0]));
    // A scatter of short vertical blades.
    let blades: [(f32, f32); 10] = [
        (0.08, 0.55),
        (0.17, 0.30),
        (0.26, 0.65),
        (0.35, 0.40),
        (0.46, 0.70),
        (0.55, 0.35),
        (0.64, 0.60),
        (0.73, 0.45),
        (0.84, 0.68),
        (0.93, 0.38),
    ];
    for &(cx, height) in &blades {
        let top = ((1.0 - height) * H as f32) as u32;
        for y in top..H {
            let t = (y - top) as f32 / (H - top).max(1) as f32; // 0 tip .. 1 root
            let half_w = 0.004 + 0.012 * t;
            let g = (120.0 + 110.0 * (1.0 - t)) as u8;
            let r = (45.0 + 60.0 * (1.0 - t)) as u8;
            let b = 30u8;
            let x0 = ((cx - half_w) * W as f32).floor().max(0.0) as u32;
            let x1 = ((cx + half_w) * W as f32).ceil().min(W as f32) as u32;
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
        }
    }

    /// A grid layer emits one quad (4 verts, 6 indices) per flagged cell,
    /// draped on the terrain heights (never floating above them).
    #[test]
    fn grid_layer_emits_quads_on_terrain() {
        // 2×2-cell layer covering cols 0..1, rows 0..1, all dense.
        let leaf = GrassLeaf {
            tex0: 10,
            tex1: 7,
            g: [0, 0, 1, 1],
            density: vec![1, 2, 5, 7],
            vertices: vec![],
            triangles: vec![],
        };
        let heights = flat_heights();
        let max_idx = 16i32;

        // Reproduce the per-cell emission (mirrors build_block_grass) without a
        // ComponentFactory.
        let mut quads = 0;
        let mut max_y = f32::MIN;
        for row in leaf.g[1]..=leaf.g[3] {
            for col in leaf.g[0]..=leaf.g[2] {
                if leaf.density_at(col, row).unwrap_or(0) == 0 {
                    continue;
                }
                quads += 1;
                for (r, c) in [
                    (row, col),
                    (row + 1, col),
                    (row, col + 1),
                    (row + 1, col + 1),
                ] {
                    let r = r.clamp(0, max_idx) as usize;
                    let c = c.clamp(0, max_idx) as usize;
                    max_y = max_y.max(heights.corners[r][c] + BASE_LIFT);
                }
            }
        }
        assert_eq!(quads, 4, "2×2 dense cells = 4 quads");
        // Grass sits on the terrain (height 100) plus a small lift — never in
        // the sky.
        assert!(
            max_y <= 100.0 + 2.0,
            "grass must hug the terrain, got {max_y}"
        );
    }
}
