//! PAL5 `.mp` terrain heightfield decoder.
//!
//! Each map block ships `Map/<map>/<map>_0_0.mp`: a GameBox container
//! (`magic = 0x0001e240`, version 20) whose body is a single zlib
//! stream. The inflated body is a sequence of **per-patch records**, one
//! per 320×320 world-unit terrain patch. Each patch is a 17×17 vertex
//! grid (16×16 cells, 20 units/cell) with shared edges between
//! neighbours.
//!
//! ## Record layout (floats, little-endian)
//! The fixed head of every record — **textured or not** — is 1458 floats:
//!
//! | offset | count | field |
//! |--------|-------|-------|
//! | 0      | 289   | per-vertex texture-layer index (`-1.0` = none, else `0..N`) |
//! | 289    | 13    | metadata: `[8]`=minX `[10]`=minZ `[5]`=maxX `[7]`=maxZ (bbox) |
//! | 302    | 289   | per-vertex height (Y) |
//! | 591    | 867   | per-vertex normal, interleaved `(nx,ny,nz)` ×289 |
//!
//! Textured patches append a small **variable-length tail** after this
//! fixed head (per-layer blend data, ~3..230 floats) whose exact size is
//! engine-internal. The geometry we need (heights at +302, normals at
//! +591) lives at the same fixed offsets in *every* record, so we decode
//! all of them and only the tail is skipped.
//!
//! Because the tail makes the per-record stride variable, the parser
//! **scans for each record-start signature** — a 320-aligned 320×320
//! bbox at `+289` followed by plausible heights — which is robust against
//! the variable tail. Earlier revisions additionally required the layer
//! field to be entirely `-1.0`, which silently dropped every textured
//! patch (≈23% of a map like `kuangfengzhai`) and left holes that had to
//! be interpolated; the bbox signature alone recovers them. Validated by
//! inter-patch edge-height continuity (shared-edge heights of textured
//! patches match their neighbours to <0.5u). See
//! `generated/pal5_tree_texture.md` and the session `mp_re_findings.md`.

use serde::Serialize;

const REC_HEAD_FLOATS: usize = 1458;
const PATCH_EDGE: usize = 17; // vertices per patch edge
const PATCH_VERTS: usize = PATCH_EDGE * PATCH_EDGE; // 289
const META_OFF: usize = 289;
const HEIGHT_OFF: usize = 302;
const NORMAL_OFF: usize = 591;

/// Header magic shared by PAL5 GameBox containers (`.mp`/`.nod`/`.env`).
const GAMEBOX_MAGIC: u32 = 0x0001_e240;

/// World size of one terrain patch edge, in game units.
pub const PATCH_WORLD_SIZE: f32 = 320.0;
/// World distance between adjacent vertices within a patch (`320 / 16`).
pub const CELL_WORLD_SIZE: f32 = PATCH_WORLD_SIZE / 16.0;

#[derive(Debug, Clone, Serialize)]
pub struct MpVertexNormal {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// One decoded terrain patch: a 17×17 vertex grid rooted at
/// `(min_x, min_z)` in world space.
#[derive(Debug, Clone, Serialize)]
pub struct MpPatch {
    pub min_x: f32,
    pub min_z: f32,
    /// Per-vertex height, row-major `[row * 17 + col]` (`row` along +Z,
    /// `col` along +X).
    pub heights: Vec<f32>,
    /// Per-vertex normal, same indexing as [`MpPatch::heights`].
    pub normals: Vec<MpVertexNormal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MpFile {
    pub patches: Vec<MpPatch>,
    /// The block's four terrain-texture slots (footer of the decompressed
    /// stream): `texture_ids[slot]` is the `TerrainTexture` package-order
    /// index for layer `slot`, or `-1` if the slot is unused. Pairs with
    /// the per-slot weight rasters in the matching `alphamap_<r>_<c>.alp`
    /// (see [`crate::pal5::alp`]). `[-1; 4]` if no footer was found.
    pub texture_ids: [i32; 4],
}

#[derive(thiserror::Error, Debug)]
pub enum MpError {
    #[error("not a GameBox container (bad magic {0:#x})")]
    BadMagic(u32),
    #[error("file too small")]
    TooSmall,
    #[error("zlib stream not found")]
    NoZlib,
    #[error("decompression failed: {0}")]
    Inflate(String),
}

fn read_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

impl MpFile {
    /// Decode a raw `.mp` file (GameBox header + zlib body).
    pub fn read(raw: &[u8]) -> Result<MpFile, MpError> {
        if raw.len() < 0x40 {
            return Err(MpError::TooSmall);
        }
        let magic = read_u32(raw, 0).ok_or(MpError::TooSmall)?;
        if magic != GAMEBOX_MAGIC {
            return Err(MpError::BadMagic(magic));
        }

        // The zlib stream begins right after the fixed GameBox header.
        // Locate it by its `78 9c` signature so we tolerate small header
        // variations; the canonical offset is 0x3c.
        let zpos = raw
            .windows(2)
            .position(|w| w == [0x78, 0x9c])
            .ok_or(MpError::NoZlib)?;
        let body = miniz_oxide::inflate::decompress_to_vec_zlib(&raw[zpos..])
            .map_err(|e| MpError::Inflate(format!("{:?}", e)))?;

        Ok(MpFile {
            patches: parse_patches(&body),
            texture_ids: parse_block_texture_ids(&body),
        })
    }
}

/// Extract the block's four terrain-texture slot ids from the footer of the
/// decompressed `.mp` stream. The footer shape (PAL5 v20) is:
/// ```text
/// i32 texture_id[4];   // -1 = unused, else TerrainTexture package index
/// i32 aux_count;
/// (i32 a, i32 b) × aux_count;  i32 trailing;   // present iff aux_count != 0
/// ```
/// We scan from the end for the largest `aux_count` whose count field is
/// self-consistent and whose four ids are all in `-1..=224`. Returns
/// `[-1; 4]` if no plausible footer is found. Derived clean-room from the
/// shipped loader (`Pal5.exe` `0x0077bfb5`..`0x0077c123`).
fn parse_block_texture_ids(body: &[u8]) -> [i32; 4] {
    let words = body.len() / 4;
    let i32_at = |w: usize| -> i32 {
        let o = w * 4;
        i32::from_le_bytes([body[o], body[o + 1], body[o + 2], body[o + 3]])
    };
    for aux_count in (0..=512usize).rev() {
        let extra = 5 + aux_count * 2 + usize::from(aux_count != 0);
        if extra > words {
            continue;
        }
        let pos = words - extra;
        let ids = [
            i32_at(pos),
            i32_at(pos + 1),
            i32_at(pos + 2),
            i32_at(pos + 3),
        ];
        let count = i32_at(pos + 4);
        if count == aux_count as i32 && ids.iter().all(|&id| (-1..=224).contains(&id)) {
            return ids;
        }
    }
    [-1; 4]
}

/// Reinterpret the inflated body as `f32`s and walk the variable-length
/// per-patch records, keying on the record-start signature.
fn parse_patches(body: &[u8]) -> Vec<MpPatch> {
    let nf = body.len() / 4;
    let f = |i: usize| -> f32 {
        let o = i * 4;
        f32::from_le_bytes([body[o], body[o + 1], body[o + 2], body[o + 3]])
    };

    let mut patches = Vec::new();
    // Scan the whole body for record-start signatures. A sequential
    // record-to-record walk is not possible because textured patches carry
    // a variable-length tail after the fixed 1458-float head, so the stride
    // is not constant. Keying on the record-start signature (a 320-aligned
    // 320×320 bbox at `+289` with plausible heights) recovers every patch —
    // textured and untextured alike — independently of the tail. The
    // geometry (heights at `+302`, normals at `+591`) shares the same fixed
    // layout in every record, so both kinds decode identically.
    let mut o = 0usize;
    while o + NORMAL_OFF <= nf {
        if !is_record_start(&f, o, nf) {
            o += 1;
            continue;
        }
        let min_x = f(o + META_OFF + 8);
        let min_z = f(o + META_OFF + 10);

        let mut heights = Vec::with_capacity(PATCH_VERTS);
        for v in 0..PATCH_VERTS {
            heights.push(f(o + HEIGHT_OFF + v));
        }
        let mut normals = Vec::with_capacity(PATCH_VERTS);
        for v in 0..PATCH_VERTS {
            let b = o + NORMAL_OFF + v * 3;
            normals.push(MpVertexNormal {
                x: f(b),
                y: f(b + 1),
                z: f(b + 2),
            });
        }
        patches.push(MpPatch {
            min_x,
            min_z,
            heights,
            normals,
        });

        // Every record is at least one fixed head (`REC_HEAD_FLOATS`) long,
        // so advancing by that amount never overshoots the next record start
        // (textured records only add a tail on top) while skipping this
        // record's interior to avoid re-matching the bbox inside the height
        // field.
        o += REC_HEAD_FLOATS;
    }

    // Keep the first occurrence of each cell (guards against any residual
    // false-positive signature landing on an already-seen origin).
    let mut seen = std::collections::HashSet::new();
    patches.retain(|p| seen.insert((p.min_x.to_bits(), p.min_z.to_bits())));

    patches
}

/// Whether offset `o` (in floats) begins a patch record (textured or not):
/// a 320-aligned 320×320 bbox in the metadata block at `+289` and plausible
/// heights at `+302`. The bbox — two coordinates exactly 320 apart on a
/// 320-unit grid — is a strong, specific signature that the height/normal
/// payload effectively never reproduces by chance, so (unlike the earlier
/// "layer field is entirely -1.0" gate) it admits the textured patches too
/// without introducing phantom matches.
fn is_record_start(f: &impl Fn(usize) -> f32, o: usize, nf: usize) -> bool {
    if o + NORMAL_OFF > nf {
        return false;
    }
    // Bounding box: 320×320, axis-origin a multiple of 320, in range.
    let min_x = f(o + META_OFF + 8);
    let min_z = f(o + META_OFF + 10);
    let max_x = f(o + META_OFF + 5);
    let max_z = f(o + META_OFF + 7);
    if !(0.0..=20000.0).contains(&min_x) || !(0.0..=40000.0).contains(&min_z) {
        return false;
    }
    if (max_x - min_x - PATCH_WORLD_SIZE).abs() > 0.5
        || (max_z - min_z - PATCH_WORLD_SIZE).abs() > 0.5
    {
        return false;
    }
    if min_x % PATCH_WORLD_SIZE != 0.0 || min_z % PATCH_WORLD_SIZE != 0.0 {
        return false;
    }
    // Heights plausible (terrain Y stays within a sane band).
    for v in (0..PATCH_VERTS).step_by(37) {
        let h = f(o + HEIGHT_OFF + v);
        if !(-2000.0..=5000.0).contains(&h) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic inflated body containing `n` header pad floats
    /// followed by one untextured patch record, then wrap it as a
    /// GameBox `.mp` (header + zlib body) and decode it.
    #[test]
    fn decodes_single_untextured_patch() {
        let mut body: Vec<f32> = Vec::new();
        // Header pad (kept short; the parser scans for the first record).
        body.extend(std::iter::repeat(0.0).take(8));

        // One record (1458 floats).
        let mut rec = vec![0.0f32; REC_HEAD_FLOATS];
        // layer: all -1
        for v in 0..PATCH_VERTS {
            rec[v] = -1.0;
        }
        // meta bbox: minX=320, minZ=640, maxX=640, maxZ=960
        rec[META_OFF + 8] = 320.0; // minX
        rec[META_OFF + 10] = 640.0; // minZ
        rec[META_OFF + 5] = 640.0; // maxX
        rec[META_OFF + 7] = 960.0; // maxZ
        // heights: ramp 0..288
        for v in 0..PATCH_VERTS {
            rec[HEIGHT_OFF + v] = v as f32;
        }
        // normals: straight up
        for v in 0..PATCH_VERTS {
            rec[NORMAL_OFF + v * 3] = 0.0;
            rec[NORMAL_OFF + v * 3 + 1] = 1.0;
            rec[NORMAL_OFF + v * 3 + 2] = 0.0;
        }
        body.extend_from_slice(&rec);
        // Trailing pad so the record isn't at the very end.
        body.extend(std::iter::repeat(0.0).take(16));

        let body_bytes: Vec<u8> = body.iter().flat_map(|f| f.to_le_bytes()).collect();
        let zlib = miniz_oxide::deflate::compress_to_vec_zlib(&body_bytes, 6);

        // GameBox header: magic + 14 u32 of padding, then the zlib body.
        let mut file = Vec::new();
        file.extend_from_slice(&GAMEBOX_MAGIC.to_le_bytes());
        file.extend(std::iter::repeat(0u8).take(0x3c - 4));
        file.extend_from_slice(&zlib);

        let mp = MpFile::read(&file).expect("decode");
        assert_eq!(mp.patches.len(), 1);
        let p = &mp.patches[0];
        assert_eq!(p.min_x, 320.0);
        assert_eq!(p.min_z, 640.0);
        assert_eq!(p.heights.len(), PATCH_VERTS);
        assert_eq!(p.heights[0], 0.0);
        assert_eq!(p.heights[288], 288.0);
        assert!((p.normals[0].y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut file = vec![0u8; 0x40];
        file[0..4].copy_from_slice(&0xdead_beefu32.to_le_bytes());
        assert!(matches!(MpFile::read(&file), Err(MpError::BadMagic(_))));
    }
}
