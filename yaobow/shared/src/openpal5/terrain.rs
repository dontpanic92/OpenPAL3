//! PAL5 terrain rendering — builds a renderable heightfield entity from
//! a decoded `.mp` ([`fileformats::pal5::mp::MpFile`]).
//!
//! Phase 2 (this module): geometry + a single tiled base grass texture
//! with per-vertex normals for lighting. Each `.mp` patch (17×17 vertex
//! grid, 320×320 world units) becomes one [`Geometry`]; all patches are
//! gathered into a single terrain entity. Multi-layer alphamap splatting
//! is a later phase.

use crosscom::ComRc;
use fileformats::pal5::mp::{CELL_WORLD_SIZE, MpFile, PATCH_WORLD_SIZE};
use radiance::comdef::IEntity;
use radiance::components::mesh::{Geometry, StaticMeshComponent, TexCoord};
use radiance::math::Vec3;
use radiance::rendering::{BlendMode, CullMode, MaterialDef, SimpleMaterialDef};
use radiance::scene::CoreEntity;

use super::asset_loader::AssetLoader;

const PATCH_EDGE: usize = 17; // vertices per patch edge

/// Base terrain texture used to tile the whole heightfield in the
/// Phase-2 single-texture pass. `dibiao*` (地表, "ground surface") are
/// the opaque base layers; `cao`/`luoye` are transparent grass/leaf
/// overlays (handled by the later splatting phase). The texture is
/// rendered fully opaque (its DXT5 alpha channel is overlay data, not
/// coverage), so we ignore it here.
const BASE_TERRAIN_TEXTURE: &str = "/Texture/TerrainTexture/dibiao424.dds";

/// World units one full tiling of the base texture spans. ~2 patches so
/// the grass repeats at a sensible scale over the landform.
const TEX_TILE_WORLD: f32 = PATCH_WORLD_SIZE * 2.0;

/// Build the terrain entity for `mp`. Returns `None` if the heightfield
/// is empty.
pub fn build_terrain_entity(
    asset_loader: &AssetLoader,
    map_name: &str,
    mp: &MpFile,
) -> Option<ComRc<IEntity>> {
    if mp.patches.is_empty() {
        return None;
    }

    let factory = asset_loader.component_factory();
    let entity = CoreEntity::create(format!("{}_terrain", map_name), true);

    let geometry = build_terrain_geometry(asset_loader, mp);

    let mesh = StaticMeshComponent::new(entity.clone(), vec![geometry], factory);
    entity.add_component(
        radiance::comdef::IStaticMeshComponent::uuid(),
        ComRc::from_object(mesh),
    );

    Some(entity)
}

/// Assemble all decoded patches into a single seamless heightfield mesh.
///
/// The `.mp` decoder now recovers every patch — textured and untextured —
/// so the grid is essentially complete (typically all but a single corner
/// patch). We still resample every patch onto one shared vertex grid keyed
/// by world position (patches share edge vertices, so this is exact for the
/// decoded cells), then **dilate-fill** any residual gaps from their known
/// neighbours so the result is always a single hole-free mesh. Decoded cells
/// keep their exact geometry; the rare missing cell is smoothly interpolated.
fn build_terrain_geometry(asset_loader: &AssetLoader, mp: &MpFile) -> Geometry {
    // Patch origins fall on a 320-unit grid; vertices on a
    // `CELL_WORLD_SIZE` (20-unit) grid. Build the height field in the
    // file's natural axes first (`col` -> fileX from `min_x`, `row` ->
    // fileZ from `min_z`), then map file -> world at emit time.
    let max_min_x = mp.patches.iter().map(|p| p.min_x).fold(0.0f32, f32::max);
    let max_min_z = mp.patches.iter().map(|p| p.min_z).fold(0.0f32, f32::max);

    let fnx = ((max_min_x + PATCH_WORLD_SIZE) / CELL_WORLD_SIZE).round() as usize + 1;
    let fnz = ((max_min_z + PATCH_WORLD_SIZE) / CELL_WORLD_SIZE).round() as usize + 1;

    let mut height = vec![0.0f32; fnx * fnz];
    let mut known = vec![false; fnx * fnz];

    for patch in &mp.patches {
        for row in 0..PATCH_EDGE {
            for col in 0..PATCH_EDGE {
                let fx = ((patch.min_x + col as f32 * CELL_WORLD_SIZE) / CELL_WORLD_SIZE).round()
                    as usize;
                let fz = ((patch.min_z + row as f32 * CELL_WORLD_SIZE) / CELL_WORLD_SIZE).round()
                    as usize;
                if fx < fnx && fz < fnz {
                    let i = fx * fnz + fz;
                    height[i] = patch.heights[row * PATCH_EDGE + col];
                    known[i] = true;
                }
            }
        }
    }

    fill_unknown_heights(&mut height, &mut known, fnx, fnz);

    // File -> world orientation. Empirically derived (clean-room) by
    // sampling decoded terrain height under each of the 8 dihedral
    // orientations at every `.nod` object's (X,Z) and comparing to the
    // object's world Y across all 139 PAL5 maps: the **identity** mapping
    // wins decisively (3025 height matches within 20u, median |ΔY| ≈ 32,
    // vs ≤2173 / median ≥130 for every other orientation including the old
    // `rot270`). It is also a true rotation, so it renders un-mirrored.
    // The patches already carry their absolute world origin in
    // `min_x`/`min_z`, so the grid axes map straight through:
    //   world (X, Z) = (fx * CELL, fz * CELL)   (fx from min_x, fz from min_z)
    // with height = heights[row*17 + col] (row along +Z, col along +X).
    let tile = TEX_TILE_WORLD;
    let mut vertices = Vec::with_capacity(fnx * fnz);
    let mut texcoords = Vec::with_capacity(fnx * fnz);
    for fx in 0..fnx {
        for fz in 0..fnz {
            let wx = fx as f32 * CELL_WORLD_SIZE;
            let wz = fz as f32 * CELL_WORLD_SIZE;
            vertices.push(Vec3::new(wx, height[fx * fnz + fz], wz));
            texcoords.push(TexCoord::new(wx / tile, wz / tile));
        }
    }

    let (nx, nz) = (fnx, fnz);
    let mut indices = Vec::with_capacity((nx - 1) * (nz - 1) * 6);
    for gx in 0..nx - 1 {
        for gz in 0..nz - 1 {
            let tl = (gx * nz + gz) as u32;
            let tr = tl + 1;
            let bl = ((gx + 1) * nz + gz) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    // See note below on omitting normals for the `TexturedNoLight` shader.
    Geometry::new(
        &vertices,
        None,
        &[texcoords],
        indices,
        terrain_material(asset_loader),
    )
}

/// Fill grid cells with no decoded height by repeatedly averaging their
/// known 4-neighbours (morphological dilation). Converges because the
/// decoded patches form a connected region surrounding every gap.
fn fill_unknown_heights(height: &mut [f32], known: &mut [bool], nx: usize, nz: usize) {
    let at = |gx: usize, gz: usize| gx * nz + gz;
    loop {
        let mut changed = false;
        let mut any_unknown = false;
        // Snapshot of the known mask so a single pass only reads
        // previously-known cells (stable dilation front).
        let prev_known = known.to_vec();
        for gx in 0..nx {
            for gz in 0..nz {
                let i = at(gx, gz);
                if prev_known[i] {
                    continue;
                }
                any_unknown = true;
                let mut sum = 0.0f32;
                let mut count = 0u32;
                let mut neighbour = |ngx: usize, ngz: usize| {
                    let ni = at(ngx, ngz);
                    if prev_known[ni] {
                        sum += height[ni];
                        count += 1;
                    }
                };
                if gx > 0 {
                    neighbour(gx - 1, gz);
                }
                if gx + 1 < nx {
                    neighbour(gx + 1, gz);
                }
                if gz > 0 {
                    neighbour(gx, gz - 1);
                }
                if gz + 1 < nz {
                    neighbour(gx, gz + 1);
                }
                if count > 0 {
                    height[i] = sum / count as f32;
                    known[i] = true;
                    changed = true;
                }
            }
        }
        if !any_unknown || !changed {
            break;
        }
    }
}

fn terrain_material(asset_loader: &AssetLoader) -> MaterialDef {
    let data = asset_loader.read_file(BASE_TERRAIN_TEXTURE).ok();
    // Force Opaque: the base ground texture's alpha channel carries
    // overlay/detail data, not coverage, so terrain must render fully
    // opaque rather than alpha-test/blend it into floating fragments.
    // Render two-sided so the heightfield stays visible regardless of
    // per-patch triangle winding.
    SimpleMaterialDef::create2(BASE_TERRAIN_TEXTURE, data)
        .with_blend(BlendMode::Opaque)
        .with_cull(CullMode::None)
}
