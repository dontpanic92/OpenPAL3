//! PAL5 terrain rendering — builds a renderable, multi-layer splatted
//! heightfield from decoded map blocks ([`super::asset_loader::MapBlock`]).
//!
//! Each map block (`<map>_<r>_<c>.mp` + `alphamap_<r>_<c>.alp`) becomes one
//! [`Geometry`]: a 16×16-patch heightfield (257×257 vertices, 20 world
//! units/cell) textured with a [`TerrainSplatMaterialDef`] that blends the
//! block's up-to-four terrain textures per-texel by a weight atlas.
//!
//! ## Data flow
//! * Heights + per-vertex normals come from the `.mp` patches (absolute
//!   world origins, so blocks tile seamlessly).
//! * The four layer texture ids come from the `.mp` block footer
//!   ([`fileformats::pal5::mp::MpFile::texture_ids`]); `-1` = unused.
//! * Per-texel layer weights come from the `.alp` patches
//!   ([`fileformats::pal5::alp`]): slot `s`'s 64×64 raster, in slot order.
//!   They are packed into one `1024×1024` RGBA weight atlas per block (one
//!   `64×64` tile per patch; R/G/B/A = slots 0/1/2/3).
//!
//! Weight that lands on an **unused** slot (texture id `-1`, observed up to
//! ~12% on some blocks) is folded into the base layer so the shader never
//! samples an undefined texture — the dominant layer is always a valid
//! slot, so this only collapses minor overlay detail into the base.
//!
//! The terrain textures are loaded **opaque** (their `.dds` alpha is
//! non-coverage detail data; left as coverage it premultiplies the RGB
//! toward black). The weight atlas is loaded **raw** so its four channels
//! survive intact.

use crosscom::ComRc;
use fileformats::pal5::alp::{WEIGHT_EDGE, terrain_texture_name};
use fileformats::pal5::mp::{CELL_WORLD_SIZE, MpFile, PATCH_WORLD_SIZE};
use image::{Rgba, RgbaImage};
use radiance::comdef::IEntity;
use radiance::components::mesh::{Geometry, StaticMeshComponent, TexCoord};
use radiance::math::Vec3;
use radiance::rendering::{MaterialDef, TerrainLayer, TerrainSplatMaterialDef};
use radiance::scene::CoreEntity;

use super::asset_loader::{AssetLoader, MapBlock};

const PATCH_EDGE: usize = 17; // vertices per patch edge
const PATCHES_PER_BLOCK: usize = 16; // patches per block edge
const CELLS_PER_BLOCK: usize = PATCHES_PER_BLOCK * 16; // 256 cells per block edge
const VERTS_PER_BLOCK: usize = CELLS_PER_BLOCK + 1; // 257 vertices per block edge
/// World size of one terrain block edge (`16 patches × 320`).
const BLOCK_WORLD_SIZE: f32 = PATCH_WORLD_SIZE * PATCHES_PER_BLOCK as f32;
/// Weight-atlas edge in texels (`16 patches × 64`).
const ATLAS_EDGE: usize = PATCHES_PER_BLOCK * WEIGHT_EDGE; // 1024

/// Fallback base texture when a block has no valid footer texture id.
const FALLBACK_TEXTURE: &str = "dibiao424.dds";

/// World units per full repeat of each ground texture. PAL5 ground textures
/// tile across the terrain; one repeat per 320-unit patch matches the
/// original (each 512² texture then resolves to ~0.6 units/texel).
const TEX_TILE_WORLD: f32 = PATCH_WORLD_SIZE;

/// Build the terrain entity for `map_name` from its decoded blocks. Returns
/// `None` if no block produced geometry.
pub fn build_terrain_entity(
    asset_loader: &AssetLoader,
    map_name: &str,
    blocks: &[MapBlock],
) -> Option<ComRc<IEntity>> {
    let factory = asset_loader.component_factory();
    let entity = CoreEntity::create(format!("{}_terrain", map_name), true);

    let geometries: Vec<Geometry> = blocks
        .iter()
        .filter_map(|block| build_block_geometry(asset_loader, map_name, block))
        .collect();
    if geometries.is_empty() {
        return None;
    }

    let mesh = StaticMeshComponent::new(entity.clone(), geometries, factory);
    entity.add_component(
        radiance::comdef::IStaticMeshComponent::uuid(),
        ComRc::from_object(mesh),
    );
    Some(entity)
}

/// Build one block's splat geometry, or `None` if the block is empty.
fn build_block_geometry(
    asset_loader: &AssetLoader,
    map_name: &str,
    block: &MapBlock,
) -> Option<Geometry> {
    let mp = &block.mp;
    if mp.patches.is_empty() {
        return None;
    }

    // Block origin in world space: snap the minimum patch origin down to the
    // block grid (patch origins are absolute; a block spans 5120 units).
    let (block_min_x, block_min_z) = block_origin(block);

    let (height, normal, _known) = build_block_height_field(mp, block_min_x, block_min_z);
    let atlas = build_weight_atlas(block, mp.texture_ids, block_min_x, block_min_z);
    let material = block_material(asset_loader, map_name, block, atlas);

    // Emit the 257×257 vertex grid. `known` cells with no decoded height are
    // dilate-filled (notably the block's omitted (0,0) patch).
    let n = VERTS_PER_BLOCK;
    let mut vertices = Vec::with_capacity(n * n);
    let mut normals = Vec::with_capacity(n * n);
    let mut texcoords = Vec::with_capacity(n * n);
    for gx in 0..n {
        for gz in 0..n {
            let wx = block_min_x + gx as f32 * CELL_WORLD_SIZE;
            let wz = block_min_z + gz as f32 * CELL_WORLD_SIZE;
            let i = gx * n + gz;
            vertices.push(Vec3::new(wx, height[i], wz));
            normals.push(normal[i]);
            // Weight-atlas UV: block-local position normalized to [0,1].
            texcoords.push(TexCoord::new(
                gx as f32 / CELLS_PER_BLOCK as f32,
                gz as f32 / CELLS_PER_BLOCK as f32,
            ));
        }
    }

    let mut indices = Vec::with_capacity((n - 1) * (n - 1) * 6);
    for gx in 0..n - 1 {
        for gz in 0..n - 1 {
            let tl = (gx * n + gz) as u32;
            let tr = tl + 1;
            let bl = ((gx + 1) * n + gz) as u32;
            let br = bl + 1;
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    Some(Geometry::new(
        &vertices,
        Some(&normals),
        &[texcoords],
        indices,
        material,
    ))
}

/// Block grass-grid heights: the terrain surface sampled at the `.ctr` grass
/// grid resolution (a `17×17` lattice of `320`-unit patch corners) plus the
/// block's world origin. Consumed by [`super::grass`] to drape the grass
/// overlay on the terrain. Indexed `corners[row][col]` where `row` steps
/// world **X** and `col` steps world **Z** (matching the `.ctr` grid index
/// `(S+1)*row + col`).
pub struct BlockGrassHeights {
    pub min_x: f32,
    pub min_z: f32,
    /// Edge length of one grass cell in world units (`320`).
    pub cell_world: f32,
    /// `17×17` patch-corner heights, `corners[row][col]`.
    pub corners: Vec<[f32; GRASS_VERTS_PER_BLOCK]>,
}

/// Grass grid cells per block edge (one per terrain patch).
const GRASS_CELLS_PER_BLOCK: usize = PATCHES_PER_BLOCK; // 16
/// Grass grid vertices per block edge.
const GRASS_VERTS_PER_BLOCK: usize = GRASS_CELLS_PER_BLOCK + 1; // 17

/// Compute a block's world origin (minimum patch origin snapped to the block
/// grid). Shared by the terrain and grass builders so they align exactly.
fn block_origin(block: &MapBlock) -> (f32, f32) {
    let mp = &block.mp;
    let min_x = mp
        .patches
        .iter()
        .map(|p| p.min_x)
        .fold(f32::MAX, f32::min)
        .min(block.row as f32 * BLOCK_WORLD_SIZE);
    let min_z = mp
        .patches
        .iter()
        .map(|p| p.min_z)
        .fold(f32::MAX, f32::min)
        .min(block.col as f32 * BLOCK_WORLD_SIZE);
    (
        (min_x / BLOCK_WORLD_SIZE).floor() * BLOCK_WORLD_SIZE,
        (min_z / BLOCK_WORLD_SIZE).floor() * BLOCK_WORLD_SIZE,
    )
}

/// Sample the block's terrain heightfield at the grass grid corners so the
/// grass overlay sits exactly on the ground. Returns `None` for empty blocks.
pub fn build_block_grass_heights(block: &MapBlock) -> Option<BlockGrassHeights> {
    if block.mp.patches.is_empty() {
        return None;
    }
    let (min_x, min_z) = block_origin(block);
    let (height, _normal, _known) = build_block_height_field(&block.mp, min_x, min_z);
    let n = VERTS_PER_BLOCK; // 257
    let step = CELLS_PER_BLOCK / GRASS_CELLS_PER_BLOCK; // 256/16 = 16 terrain cells

    let mut corners = vec![[0.0f32; GRASS_VERTS_PER_BLOCK]; GRASS_VERTS_PER_BLOCK];
    for row in 0..GRASS_VERTS_PER_BLOCK {
        for col in 0..GRASS_VERTS_PER_BLOCK {
            let gx = (row * step).min(n - 1);
            let gz = (col * step).min(n - 1);
            corners[row][col] = height[gx * n + gz];
        }
    }
    Some(BlockGrassHeights {
        min_x,
        min_z,
        cell_world: BLOCK_WORLD_SIZE / GRASS_CELLS_PER_BLOCK as f32, // 320
        corners,
    })
}

/// Rasterize a block's `.mp` patches into a 257×257 height + normal grid,
/// dilate-filling any cell no patch covered.
fn build_block_height_field(
    mp: &MpFile,
    block_min_x: f32,
    block_min_z: f32,
) -> (Vec<f32>, Vec<Vec3>, Vec<bool>) {
    let n = VERTS_PER_BLOCK;
    let mut height = vec![0.0f32; n * n];
    let mut normal = vec![Vec3::new(0.0, 1.0, 0.0); n * n];
    let mut known = vec![false; n * n];

    for patch in &mp.patches {
        for row in 0..PATCH_EDGE {
            for col in 0..PATCH_EDGE {
                let gx = ((patch.min_x - block_min_x + col as f32 * CELL_WORLD_SIZE)
                    / CELL_WORLD_SIZE)
                    .round() as i64;
                let gz = ((patch.min_z - block_min_z + row as f32 * CELL_WORLD_SIZE)
                    / CELL_WORLD_SIZE)
                    .round() as i64;
                if gx < 0 || gz < 0 || gx as usize >= n || gz as usize >= n {
                    continue;
                }
                let i = gx as usize * n + gz as usize;
                let v = row * PATCH_EDGE + col;
                height[i] = patch.heights[v];
                let nm = &patch.normals[v];
                normal[i] = Vec3::new(nm.x, nm.y, nm.z);
                known[i] = true;
            }
        }
    }

    fill_unknown_heights(&mut height, &mut normal, &mut known, n);
    (height, normal, known)
}

/// Build the block's `1024×1024` RGBA weight atlas (one `64×64` tile per
/// patch; R/G/B/A = slot 0/1/2/3 weights). Weight on slots whose texture id
/// is `-1` is folded into the base layer. Cells with no decoded patch get a
/// full-base weight (`R = 255`).
fn build_weight_atlas(
    block: &MapBlock,
    texture_ids: [i32; 4],
    block_min_x: f32,
    block_min_z: f32,
) -> RgbaImage {
    let mut atlas =
        RgbaImage::from_pixel(ATLAS_EDGE as u32, ATLAS_EDGE as u32, Rgba([255, 0, 0, 0]));
    let Some(alp) = block.alp.as_ref() else {
        return atlas; // base-only block
    };

    for patch in &block.mp.patches {
        let lx = ((patch.min_x - block_min_x) / PATCH_WORLD_SIZE).round() as i64;
        let lz = ((patch.min_z - block_min_z) / PATCH_WORLD_SIZE).round() as i64;
        if lx < 0 || lz < 0 || lx as usize >= PATCHES_PER_BLOCK || lz as usize >= PATCHES_PER_BLOCK
        {
            continue;
        }
        let Some(ap) = alp.patch(lx as usize, lz as usize) else {
            continue;
        };
        write_patch_weights(&mut atlas, ap, texture_ids, lx as usize, lz as usize);
    }
    atlas
}

/// Write one patch's `64×64` weights into its atlas tile, folding unused-slot
/// weight into the base layer.
fn write_patch_weights(
    atlas: &mut RgbaImage,
    ap: &fileformats::pal5::alp::AlpPatch,
    texture_ids: [i32; 4],
    lx: usize,
    lz: usize,
) {
    let base_x = lx * WEIGHT_EDGE;
    let base_z = lz * WEIGHT_EDGE;
    for row in 0..WEIGHT_EDGE {
        for col in 0..WEIGHT_EDGE {
            let t = row * WEIGHT_EDGE + col;
            let mut w = [0u32; 4];
            for slot in 0..ap.layer_count as usize {
                w[slot] = ap.planes[slot][t] as u32;
            }
            // Fold weight on unused slots (id < 0) into the base layer so the
            // shader never samples an undefined texture.
            for slot in 1..4 {
                if texture_ids[slot] < 0 {
                    w[0] += w[slot];
                    w[slot] = 0;
                }
            }
            atlas.put_pixel(
                (base_x + col) as u32,
                (base_z + row) as u32,
                Rgba([
                    w[0].min(255) as u8,
                    w[1].min(255) as u8,
                    w[2].min(255) as u8,
                    w[3].min(255) as u8,
                ]),
            );
        }
    }
}

/// Build the splat material for a block from its footer texture ids + atlas.
fn block_material(
    asset_loader: &AssetLoader,
    map_name: &str,
    block: &MapBlock,
    atlas: RgbaImage,
) -> MaterialDef {
    let ids = block.mp.texture_ids;
    // Base texture id: first valid slot, else the fallback.
    let base_name = ids
        .iter()
        .find(|&&id| id >= 0)
        .and_then(|&id| terrain_texture_name(id as u8))
        .unwrap_or(FALLBACK_TEXTURE);

    let load_layer = |id: i32| -> TerrainLayer {
        let name = if id >= 0 {
            terrain_texture_name(id as u8).unwrap_or(base_name)
        } else {
            // Unused slot: bind the base texture (its atlas weight is 0).
            base_name
        };
        let path = format!("/Texture/TerrainTexture/{}", name);
        TerrainLayer {
            name: path.clone(),
            data: asset_loader.read_file(&path).ok(),
        }
    };

    let layers = [
        load_layer(ids[0]),
        load_layer(ids[1]),
        load_layer(ids[2]),
        load_layer(ids[3]),
    ];

    let atlas_name = format!(
        "pal5_terrain_weights/{}_{}_{}",
        map_name, block.row, block.col
    );

    TerrainSplatMaterialDef::create(
        &format!("{}_terrain_{}_{}", map_name, block.row, block.col),
        layers,
        &atlas_name,
        atlas,
        4,
        TEX_TILE_WORLD,
    )
}

/// Fill grid cells with no decoded height/normal by repeatedly averaging
/// their known 4-neighbours (morphological dilation).
fn fill_unknown_heights(height: &mut [f32], normal: &mut [Vec3], known: &mut [bool], n: usize) {
    let at = |gx: usize, gz: usize| gx * n + gz;
    loop {
        let mut changed = false;
        let mut any_unknown = false;
        let prev_known = known.to_vec();
        for gx in 0..n {
            for gz in 0..n {
                let i = at(gx, gz);
                if prev_known[i] {
                    continue;
                }
                any_unknown = true;
                let mut hsum = 0.0f32;
                let mut nsum = Vec3::new(0.0, 0.0, 0.0);
                let mut count = 0u32;
                let mut neighbour = |ngx: usize, ngz: usize| {
                    let ni = at(ngx, ngz);
                    if prev_known[ni] {
                        hsum += height[ni];
                        nsum = Vec3::add(&nsum, &normal[ni]);
                        count += 1;
                    }
                };
                if gx > 0 {
                    neighbour(gx - 1, gz);
                }
                if gx + 1 < n {
                    neighbour(gx + 1, gz);
                }
                if gz > 0 {
                    neighbour(gx, gz - 1);
                }
                if gz + 1 < n {
                    neighbour(gx, gz + 1);
                }
                if count > 0 {
                    height[i] = hsum / count as f32;
                    normal[i] = Vec3::normalized(&nsum);
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
