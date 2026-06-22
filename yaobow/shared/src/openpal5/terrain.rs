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

    let geometries: Vec<Geometry> = mp
        .patches
        .iter()
        .map(|patch| build_patch_geometry(asset_loader, patch))
        .collect();

    let mesh = StaticMeshComponent::new(entity.clone(), geometries, factory);
    entity.add_component(
        radiance::comdef::IStaticMeshComponent::uuid(),
        ComRc::from_object(mesh),
    );

    Some(entity)
}

fn build_patch_geometry(
    asset_loader: &AssetLoader,
    patch: &fileformats::pal5::mp::MpPatch,
) -> Geometry {
    let mut vertices = Vec::with_capacity(PATCH_EDGE * PATCH_EDGE);
    let mut texcoords = Vec::with_capacity(PATCH_EDGE * PATCH_EDGE);

    for row in 0..PATCH_EDGE {
        for col in 0..PATCH_EDGE {
            let idx = row * PATCH_EDGE + col;
            // The `.mp` patch's `min_x` field indexes the world **Z**
            // axis and `min_z` indexes world **X** (verified by aligning
            // decoded heights against `.nod` object positions: this
            // mapping places 219/415 objects within the terrain vs 76
            // for the naive mapping). Row walks +X, column walks +Z.
            let x = patch.min_z + row as f32 * CELL_WORLD_SIZE;
            let z = patch.min_x + col as f32 * CELL_WORLD_SIZE;
            let y = patch.heights[idx];
            vertices.push(Vec3::new(x, y, z));
            texcoords.push(TexCoord::new(x / TEX_TILE_WORLD, z / TEX_TILE_WORLD));
        }
    }

    // Two triangles per cell, winding chosen so the +Y faces are
    // front-facing under the engine's CCW front-face convention.
    let mut indices = Vec::with_capacity((PATCH_EDGE - 1) * (PATCH_EDGE - 1) * 6);
    for row in 0..PATCH_EDGE - 1 {
        for col in 0..PATCH_EDGE - 1 {
            let tl = (row * PATCH_EDGE + col) as u32;
            let tr = tl + 1;
            let bl = ((row + 1) * PATCH_EDGE + col) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    // Note: `SimpleMaterialDef` uses the `TexturedNoLight` shader, whose
    // vertex layout is POSITION + TEXCOORD only. We deliberately omit
    // normals here — including a NORMAL component would change the
    // vertex stride and desync the attributes (garbage positions). The
    // decoded per-vertex normals are kept on `MpPatch` for a future lit
    // terrain shader.
    Geometry::new(
        &vertices,
        None,
        &[texcoords],
        indices,
        terrain_material(asset_loader),
    )
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
