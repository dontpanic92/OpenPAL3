//! PAL5 terrain rendering — builds a renderable, multi-layer splatted
//! heightfield from decoded map blocks ([`super::asset_loader::MapBlock`]).
//!
//! Each map block (`<map>_<r>_<c>.mp` + `alphamap_<r>_<c>.alp`) becomes one
//! [`Geometry`]: a 16×16-patch heightfield (257×257 vertices, 20 world
//! units/cell) textured with a [`TerrainSplatMaterialDef`] that composites the
//! block's up-to-four terrain textures per-texel using the original engine's
//! ordered alpha-"over" model.
//!
//! ## Blend model (reverse-engineered from `Pal5.exe`)
//! The original draws each terrain chunk as a base layer (opaque, the `.alp`
//! primary byte `b2` → slot 2) plus alpha-blended overlay layers
//! (SRCALPHA / INVSRCALPHA) in paint order `b1, b0, b3` → slots 1, 0, 3, the
//! per-vertex alpha being each layer's weight. The splat shader replicates this
//! per texel: `col = slot2; col = mix(col, slot1, w1); mix(col, slot0, w0);
//! mix(col, slot3, w3)`. This keeps dominant overlays clean instead of muddily
//! averaging all four layers (the previous normalized weighted-sum).
//!
//! ## Data flow
//! * Heights + per-vertex normals come from the `.mp` patches (absolute
//!   world origins, so blocks tile seamlessly).
//! * The four layer texture ids come from the `.mp` block footer
//!   ([`fileformats::pal5::mp::MpFile::texture_ids`]); `-1` = unused.
//! * Per-texel layer weights come from the `.alp` patches
//!   ([`fileformats::pal5::alp`]): `planes[s]` is texture slot `s`'s 64×64
//!   raster (one channel per slot, byte `b_s`). They are packed into one
//!   `1024×1024` RGBA weight atlas per block (one `64×64` tile per patch;
//!   R/G/B/A = slots 0/1/2/3). Slot 2's weight is unused by the shader (the
//!   base is unconditional); overlay weight on an **unused** slot (texture id
//!   `-1`) is zeroed so the shader never blends in the base-fallback texture.
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
pub(crate) const BLOCK_WORLD_SIZE: f32 = PATCH_WORLD_SIZE * PATCHES_PER_BLOCK as f32;
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
        .flat_map(|block| build_block_geometries(asset_loader, map_name, block))
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

// ---- PAL5 terrain texture pipeline (RE-corrected model) --------------------
//
// Each `.mp` patch carries an **overlay palette** (up to 4 `TextureID`s), a
// per-vertex overlay-`layer` assignment, and a **base ground** id (`dibiao*`);
// the matching `.alp` patch supplies per-texel RGBA blend weights. The original
// `terra{N}.psh` renders each patch as a **linear weighted sum** of its N
// overlay textures by those weights (`color = Σ tex_i * weight_i`); untextured
// patches (no palette) show the block default ground. We reproduce this with
// [`TerrainSplatMaterialDef`] (weighted-sum `terrain_splat.frag`).
//
// `.alp` byte → shader-slot mapping: the engine copies the `.alp` RGBA raster
// verbatim into an A8R8G8B8 blend texture, so `terra{N}` sees
// `blend.rgba = (b2, b1, b0, b3)` weighting palette slots 0..3 — i.e. slot `j`
// reads packed byte [`SLOT_TO_BYTE[j]`]. (`alp::AlpPatch::planes[k]` is byte
// `b_k`.) See `generated/pal5_terrain_{shader_spec,loader_re}.md`.

/// Packed-`.alp`-byte index feeding each shader slot / palette entry (from the
/// verbatim A8R8G8B8 copy: slot0←b2, slot1←b1, slot2←b0, slot3←b3).
const SLOT_TO_BYTE: [usize; 4] = [2, 1, 0, 3];

/// The four terrain-texture layer ids for a patch (slot 0 first; `-1` = unused):
/// the patch's own overlay palette ([`fileformats::pal5::mp::MpPatch::tex_table`])
/// if it carries one, else the block footer default
/// ([`fileformats::pal5::mp::MpFile::texture_ids`]). Both pair with the same
/// `.alp` RGBA weights (identity channel order, remapped by [`SLOT_TO_BYTE`]).
fn patch_layer_ids(patch: &fileformats::pal5::mp::MpPatch, footer: [i32; 4]) -> [i32; 4] {
    if patch.tex_table.iter().any(|&id| id >= 0) {
        patch.tex_table
    } else {
        footer
    }
}

/// Build every geometry for one block: one per distinct material group
/// (overlay palette / default ground). Each patch is drawn exactly once as a
/// weighted-sum splat — no base-grid vs. road split.
fn build_block_geometries(
    asset_loader: &AssetLoader,
    map_name: &str,
    block: &MapBlock,
) -> Vec<Geometry> {
    let mp = &block.mp;
    if mp.patches.is_empty() {
        return vec![];
    }
    let (block_min_x, block_min_z) = block_origin(block);
    let footer = mp.texture_ids;

    // Group patches by their 4 layer-texture ids (own palette or footer) so each
    // distinct material is one draw.
    let mut groups: Vec<([i32; 4], Vec<usize>)> = Vec::new();
    for (pi, patch) in mp.patches.iter().enumerate() {
        let key = patch_layer_ids(patch, footer);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, idxs)) => idxs.push(pi),
            None => groups.push((key, vec![pi])),
        }
    }

    let mut geometries = Vec::new();
    for (gi, (layer_ids, patch_idxs)) in groups.iter().enumerate() {
        if let Some(g) = build_group_geometry(
            asset_loader,
            map_name,
            block,
            block_min_x,
            block_min_z,
            *layer_ids,
            patch_idxs,
            gi,
        ) {
            geometries.push(g);
        }
    }
    geometries
}

/// Build one material group's geometry: every patch's 17×17 grid plus a weight
/// atlas whose per-patch tile carries the `.alp` blend weights for the group's
/// four layer textures.
#[allow(clippy::too_many_arguments)]
fn build_group_geometry(
    asset_loader: &AssetLoader,
    map_name: &str,
    block: &MapBlock,
    block_min_x: f32,
    block_min_z: f32,
    layer_ids: [i32; 4],
    patch_idxs: &[usize],
    group_idx: usize,
) -> Option<Geometry> {
    let mp = &block.mp;

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut texcoords = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut atlas = RgbaImage::from_pixel(ATLAS_EDGE as u32, ATLAS_EDGE as u32, Rgba([0, 0, 0, 0]));

    for &pi in patch_idxs {
        let patch = &mp.patches[pi];
        let lx = ((patch.min_x - block_min_x) / PATCH_WORLD_SIZE).round() as i64;
        let lz = ((patch.min_z - block_min_z) / PATCH_WORLD_SIZE).round() as i64;
        if !(0..PATCHES_PER_BLOCK as i64).contains(&lx)
            || !(0..PATCHES_PER_BLOCK as i64).contains(&lz)
        {
            continue;
        }
        let (lx, lz) = (lx as usize, lz as usize);

        // Weight tile from the `.alp` RGBA raster (remapped by SLOT_TO_BYTE),
        // zeroing unused layer slots; fall back to the patch's own per-vertex
        // layer assignment when the block has no decoded `.alp`.
        match block.alp.as_ref().and_then(|alp| alp.patch(lx, lz)) {
            Some(ap) if ap.planes.len() >= 4 => {
                write_overlay_weights(&mut atlas, ap, layer_ids, lx, lz);
            }
            _ => write_vertex_layer_weights(&mut atlas, patch, layer_ids, lx, lz),
        }

        // Emit this patch's 17×17 vertex grid.
        let base_vert = vertices.len() as u32;
        for row in 0..PATCH_EDGE {
            for col in 0..PATCH_EDGE {
                let wx = patch.min_x + col as f32 * CELL_WORLD_SIZE;
                let wz = patch.min_z + row as f32 * CELL_WORLD_SIZE;
                let v = row * PATCH_EDGE + col;
                vertices.push(Vec3::new(wx, patch.heights[v], wz));
                let nm = &patch.normals[v];
                normals.push(Vec3::new(nm.x, nm.y, nm.z));
                // Block-local UV addresses this patch's weight-atlas tile; the
                // ground textures tile in world space in the shader.
                texcoords.push(TexCoord::new(
                    (wx - block_min_x) / (CELLS_PER_BLOCK as f32 * CELL_WORLD_SIZE),
                    (wz - block_min_z) / (CELLS_PER_BLOCK as f32 * CELL_WORLD_SIZE),
                ));
            }
        }
        for row in 0..PATCH_EDGE - 1 {
            for col in 0..PATCH_EDGE - 1 {
                let tl = base_vert + (row * PATCH_EDGE + col) as u32;
                let tr = tl + 1;
                let bl = base_vert + ((row + 1) * PATCH_EDGE + col) as u32;
                let br = bl + 1;
                indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            }
        }
    }
    if indices.is_empty() {
        return None;
    }

    let material = group_material(asset_loader, map_name, block, layer_ids, group_idx, atlas);
    Some(Geometry::new(
        &vertices,
        Some(&normals),
        &[texcoords],
        indices,
        material,
    ))
}

/// Write a textured patch's 64×64 atlas tile from its `.alp` RGBA weights,
/// mapping packed byte `b_{SLOT_TO_BYTE[slot]}` → slot and zeroing any slot
/// whose overlay id is unused (`-1`).
fn write_overlay_weights(
    atlas: &mut RgbaImage,
    ap: &fileformats::pal5::alp::AlpPatch,
    layer_ids: [i32; 4],
    lx: usize,
    lz: usize,
) {
    let base_x = lx * WEIGHT_EDGE;
    let base_z = lz * WEIGHT_EDGE;
    for r in 0..WEIGHT_EDGE {
        for c in 0..WEIGHT_EDGE {
            let t = r * WEIGHT_EDGE + c;
            let mut w = [0u8; 4];
            for slot in 0..4 {
                if layer_ids[slot] >= 0 {
                    w[slot] = ap.planes[SLOT_TO_BYTE[slot]][t];
                }
            }
            atlas.put_pixel((base_x + c) as u32, (base_z + r) as u32, Rgba(w));
        }
    }
}

/// Fallback tile (no decoded `.alp`): one-hot weights from the patch's own
/// per-vertex overlay-layer assignment; unpainted vertices fall to slot 0.
fn write_vertex_layer_weights(
    atlas: &mut RgbaImage,
    patch: &fileformats::pal5::mp::MpPatch,
    layer_ids: [i32; 4],
    lx: usize,
    lz: usize,
) {
    let base_x = lx * WEIGHT_EDGE;
    let base_z = lz * WEIGHT_EDGE;
    for r in 0..WEIGHT_EDGE {
        for c in 0..WEIGHT_EDGE {
            let vr = (r * (PATCH_EDGE - 1) + WEIGHT_EDGE / 2) / WEIGHT_EDGE;
            let vc = (c * (PATCH_EDGE - 1) + WEIGHT_EDGE / 2) / WEIGHT_EDGE;
            let v = vr.min(PATCH_EDGE - 1) * PATCH_EDGE + vc.min(PATCH_EDGE - 1);
            let layer = patch.vert_layer[v];
            let slot = if (0..=3).contains(&layer) && layer_ids[layer as usize] >= 0 {
                layer as usize
            } else {
                0
            };
            let mut w = [0u8; 4];
            w[slot] = 255;
            atlas.put_pixel((base_x + c) as u32, (base_z + r) as u32, Rgba(w));
        }
    }
}

/// Build the splat material for a group: bind its up-to-4 layer textures + the
/// per-patch weight atlas. Unused slots repeat slot 0's texture (their atlas
/// weight is 0). `active_layers` = number of bound (non-`-1`) layers.
fn group_material(
    asset_loader: &AssetLoader,
    map_name: &str,
    block: &MapBlock,
    layer_ids: [i32; 4],
    group_idx: usize,
    atlas: RgbaImage,
) -> MaterialDef {
    let base_name = layer_ids
        .iter()
        .find(|&&id| id >= 0)
        .and_then(|&id| terrain_texture_name(id as u8))
        .unwrap_or(FALLBACK_TEXTURE);
    let load_layer = |id: i32| -> TerrainLayer {
        let name = if id >= 0 {
            terrain_texture_name(id as u8).unwrap_or(base_name)
        } else {
            base_name
        };
        let path = format!("/Texture/TerrainTexture/{}", name);
        TerrainLayer {
            name: path.clone(),
            data: asset_loader.read_file(&path).ok(),
        }
    };
    let layers = [
        load_layer(layer_ids[0]),
        load_layer(layer_ids[1]),
        load_layer(layer_ids[2]),
        load_layer(layer_ids[3]),
    ];
    let active_layers = layer_ids.iter().filter(|&&id| id >= 0).count().max(1) as u8;
    let atlas_name = format!(
        "pal5_terrain/{}_{}_{}_{}",
        map_name, block.row, block.col, group_idx
    );
    TerrainSplatMaterialDef::create(
        &format!(
            "{}_terrain_{}_{}_{}",
            map_name, block.row, block.col, group_idx
        ),
        layers,
        &atlas_name,
        atlas,
        active_layers,
        TEX_TILE_WORLD,
    )
}

/// Compute a block's world origin (minimum patch origin snapped to the block
/// grid). Shared by the terrain and grass builders so they align exactly.
pub(crate) fn block_origin(block: &MapBlock) -> (f32, f32) {
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

/// A block's 257×257 terrain height grid plus its world origin, used to drape
/// the `.ctr` grass overlay onto the ground.
pub(crate) struct BlockHeightField {
    origin_x: f32,
    origin_z: f32,
    height: Vec<f32>,
    n: usize,
}

impl BlockHeightField {
    /// The block's `(origin_x, origin_z)` world corner.
    pub(crate) fn origin(&self) -> (f32, f32) {
        (self.origin_x, self.origin_z)
    }

    /// Bilinearly sample the ground height at world `(wx, wz)`, clamped to the
    /// block's extent.
    pub(crate) fn sample(&self, wx: f32, wz: f32) -> f32 {
        let max = (self.n - 1) as f32;
        let fx = ((wx - self.origin_x) / CELL_WORLD_SIZE).clamp(0.0, max);
        let fz = ((wz - self.origin_z) / CELL_WORLD_SIZE).clamp(0.0, max);
        let x0 = fx.floor() as usize;
        let z0 = fz.floor() as usize;
        let x1 = (x0 + 1).min(self.n - 1);
        let z1 = (z0 + 1).min(self.n - 1);
        let tx = fx - x0 as f32;
        let tz = fz - z0 as f32;
        let h = |gx: usize, gz: usize| self.height[gx * self.n + gz];
        let a = h(x0, z0) + (h(x1, z0) - h(x0, z0)) * tx;
        let b = h(x0, z1) + (h(x1, z1) - h(x0, z1)) * tx;
        a + (b - a) * tz
    }
}

/// Build a block's terrain height field (origin + dilate-filled 257×257 grid)
/// for grass draping.
pub(crate) fn block_height_field(block: &MapBlock) -> BlockHeightField {
    let (origin_x, origin_z) = block_origin(block);
    let (height, _normal, _real) = build_block_height_field(&block.mp, origin_x, origin_z);
    BlockHeightField {
        origin_x,
        origin_z,
        height,
        n: VERTS_PER_BLOCK,
    }
}

#[cfg(test)]
impl BlockHeightField {
    /// A constant-height field for tests (grass overlay unit tests).
    pub(crate) fn flat_for_test(origin_x: f32, origin_z: f32, height: f32) -> BlockHeightField {
        BlockHeightField {
            origin_x,
            origin_z,
            height: vec![height; VERTS_PER_BLOCK * VERTS_PER_BLOCK],
            n: VERTS_PER_BLOCK,
        }
    }
}

/// Sample the block's terrain heightfield at the grass grid corners so the
/// grass overlay sits exactly on the ground. Returns `None` for empty blocks.
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

    // Capture the real-patch mask BEFORE dilation: `fill_unknown_heights`
    // marks every extrapolated void cell `known = true`, so the post-fill mask
    // is useless for telling real ground from filled void. Grass placement
    // needs the real mask to avoid floating tufts over the omitted region.
    let real = known.clone();
    fill_unknown_heights(&mut height, &mut normal, &mut known, n);
    (height, normal, real)
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
