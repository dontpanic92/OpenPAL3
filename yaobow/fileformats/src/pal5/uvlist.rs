//! PAL5 `Config/uvlist.tb` decoder — the sprite / foliage-card UV table.
//!
//! PAL5 renders tree leaves (and other "sprite" cards) as flat quads in the
//! tree `.dff`, tagged on the card frame with a `prt` UserData string of the
//! form `[W]…` / `[w]{t<id>}{s<pct>}<name>`. Many of those quads ship with
//! **no texture and no UV set**; the engine resolves the leaf texture + UVs at
//! load time from this table, keyed by the `{t<id>}` value. See
//! `generated/pal5_leaf_re.md` for the clean-room reverse-engineering notes
//! (`Pal5.exe.unpacked.exe`).
//!
//! ## File layout (little-endian)
//!
//! ```text
//! 0x00  char  magic[4]      // "UVL\0"
//! 0x04  u8    version[8]    // build stamp + "1.02"
//! 0x0c  u32   (records-section size hint; not required to decode)
//! 0x10  ...   records section (variable-length records, see below)
//!       ...   index table   (12-byte entries, at the tail)
//!  EOF-4 u32  trailer       // max id + 1
//! ```
//!
//! ### Index table (tail, 12-byte entries)
//! `(u32 id, u32 file_offset, u32 record_len)` — `{t<id>}` is looked up here by
//! `id`; `file_offset`/`record_len` locate the record in the records section.
//! Ids are strictly ascending, so the table is found by walking 12-byte entries
//! back from `EOF-4` while the ids stay ordered.
//!
//! ### Record (at `file_offset`, `record_len` bytes)
//! ```text
//! u32 a0                    // 0
//! u32 a1                    // 1
//! u32 frame_count           // # texture frames (1 = static; >1 = anim cycle)
//! u32 str_count             // == frame_count
//! frame_count × {
//!     f32 u0, u1, v0, v1    // UV sub-rect (leaves: 0,1,0,1 = whole atlas)
//!     u32 kind              // 10 typical
//!     u32 frame_index       // 0,1,2,…
//! }
//! str_count × {
//!     u32  tag              // per-frame tag (observed 0)
//!     u32  len
//!     char atlas_path[len]  // null-terminated, e.g. "BuildingP5\\zhiwu\\tree_yinxingqiu"
//! }
//! ```
//! Validated examples: `t6140` → 1 frame, `BuildingP5\zhiwu\tree_yinxingqiu`;
//! `t6091` → 1 frame, `BuildingP5\zhiwu\zw_tree_rs04`; `t6090` → 3 frames,
//! `zw_gushu_A`/`_C`/`_D`. Atlas textures live under `Texture\<path>.dds`.

use std::collections::HashMap;

const MAGIC: &[u8; 4] = b"UVL\0";
const HEADER_LEN: usize = 0x10;
const INDEX_ENTRY_LEN: usize = 12;
const TRAILER_LEN: usize = 4;

/// One texture frame of a UV-list entry: an atlas-relative UV sub-rectangle and
/// the atlas texture path (without the `Texture\` prefix or `.dds` suffix).
#[derive(Debug, Clone, PartialEq)]
pub struct UvFrame {
    /// UV rectangle as stored: `(u0, u1, v0, v1)`. Leaf cards use the whole
    /// atlas `(0.0, 1.0, 0.0, 1.0)`.
    pub u0: f32,
    pub u1: f32,
    pub v0: f32,
    pub v1: f32,
    /// Atlas texture path, backslash-separated as shipped
    /// (e.g. `BuildingP5\zhiwu\tree_yinxingqiu`).
    pub atlas: String,
}

/// A decoded UV-list entry: one or more texture frames. A single frame is a
/// static card; multiple frames are an animation cycle (`HaveUVAnim`).
#[derive(Debug, Clone, PartialEq)]
pub struct UvListEntry {
    pub frames: Vec<UvFrame>,
}

/// Decoded `uvlist.tb`: maps each `{t<id>}` to its [`UvListEntry`].
#[derive(Debug, Clone, Default)]
pub struct UvListFile {
    pub entries: HashMap<u32, UvListEntry>,
}

#[derive(thiserror::Error, Debug)]
pub enum UvListError {
    #[error("not a uvlist.tb (bad magic)")]
    BadMagic,
    #[error("file too small")]
    TooSmall,
    #[error("no valid index table found")]
    NoIndex,
}

fn read_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_f32(b: &[u8], off: usize) -> Option<f32> {
    b.get(off..off + 4)
        .map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

impl UvListFile {
    /// Decode a raw `uvlist.tb`. Malformed individual records are skipped so a
    /// single bad entry can't drop the whole table.
    pub fn read(raw: &[u8]) -> Result<UvListFile, UvListError> {
        if raw.len() < HEADER_LEN + TRAILER_LEN {
            return Err(UvListError::TooSmall);
        }
        if &raw[0..4] != MAGIC {
            return Err(UvListError::BadMagic);
        }

        let index_start = locate_index(raw).ok_or(UvListError::NoIndex)?;
        let index_end = raw.len() - TRAILER_LEN;

        let mut entries = HashMap::new();
        let mut o = index_start;
        while o + INDEX_ENTRY_LEN <= index_end {
            let id = read_u32(raw, o).unwrap();
            let rec_off = read_u32(raw, o + 4).unwrap() as usize;
            let rec_len = read_u32(raw, o + 8).unwrap() as usize;
            o += INDEX_ENTRY_LEN;

            if rec_off < HEADER_LEN || rec_off + rec_len > raw.len() {
                continue;
            }
            if let Some(entry) = parse_record(&raw[rec_off..rec_off + rec_len]) {
                entries.insert(id, entry);
            }
        }

        Ok(UvListFile { entries })
    }

    /// Look up the atlas texture path of a `{t<id>}` card's first frame.
    pub fn atlas_for(&self, id: u32) -> Option<&str> {
        self.entries
            .get(&id)
            .and_then(|e| e.frames.first())
            .map(|f| f.atlas.as_str())
    }
}

/// Find the start of the tail index table: walk 12-byte entries back from
/// `EOF - 4` while the ids stay strictly ascending and the `(offset, len)`
/// stay in range. Returns the offset of the first index entry.
fn locate_index(raw: &[u8]) -> Option<usize> {
    let end = raw.len().checked_sub(TRAILER_LEN)?;
    if end < HEADER_LEN + INDEX_ENTRY_LEN {
        return None;
    }
    let valid = |o: usize| -> Option<(u32, usize, usize)> {
        let id = read_u32(raw, o)?;
        let off = read_u32(raw, o + 4)? as usize;
        let len = read_u32(raw, o + 8)? as usize;
        if off >= HEADER_LEN && off + len <= raw.len() && len > 0 && len < 1 << 20 {
            Some((id, off, len))
        } else {
            None
        }
    };

    // Anchor on the last entry, then extend backwards while ids ascend.
    let mut start = end - INDEX_ENTRY_LEN;
    valid(start)?;
    while start >= HEADER_LEN + INDEX_ENTRY_LEN {
        let prev = start - INDEX_ENTRY_LEN;
        match (valid(prev), valid(start)) {
            (Some((pid, ..)), Some((cid, ..))) if pid < cid => start = prev,
            _ => break,
        }
    }
    Some(start)
}

fn parse_record(r: &[u8]) -> Option<UvListEntry> {
    let frame_count = read_u32(r, 8)? as usize;
    let str_count = read_u32(r, 12)? as usize;
    if frame_count == 0 || frame_count > 256 || str_count != frame_count {
        return None;
    }

    let mut uvs = Vec::with_capacity(frame_count);
    let mut o = 16;
    for _ in 0..frame_count {
        let u0 = read_f32(r, o)?;
        let u1 = read_f32(r, o + 4)?;
        let v0 = read_f32(r, o + 8)?;
        let v1 = read_f32(r, o + 12)?;
        // o + 16: kind, o + 20: frame_index — not needed for rendering.
        uvs.push((u0, u1, v0, v1));
        o += 24;
    }

    let mut frames = Vec::with_capacity(frame_count);
    for &(u0, u1, v0, v1) in &uvs {
        // Each string entry is `u32 (per-frame tag, observed 0) + u32 len +
        // bytes[len]` (null-terminated, zero-padded).
        o += 4;
        let len = read_u32(r, o)? as usize;
        o += 4;
        let bytes = r.get(o..o + len)?;
        o += len;
        let atlas = bytes
            .split(|&c| c == 0)
            .next()
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .unwrap_or_default();
        frames.push(UvFrame {
            u0,
            u1,
            v0,
            v1,
            atlas,
        });
    }

    Some(UvListEntry { frames })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a one-entry `uvlist.tb` and round-trip it.
    fn build(id: u32, frames: &[(f32, f32, f32, f32, &str)]) -> Vec<u8> {
        let mut rec = Vec::new();
        rec.extend_from_slice(&0u32.to_le_bytes()); // a0
        rec.extend_from_slice(&1u32.to_le_bytes()); // a1
        rec.extend_from_slice(&(frames.len() as u32).to_le_bytes()); // frame_count
        rec.extend_from_slice(&(frames.len() as u32).to_le_bytes()); // str_count
        for (i, (u0, u1, v0, v1, _)) in frames.iter().enumerate() {
            rec.extend_from_slice(&u0.to_le_bytes());
            rec.extend_from_slice(&u1.to_le_bytes());
            rec.extend_from_slice(&v0.to_le_bytes());
            rec.extend_from_slice(&v1.to_le_bytes());
            rec.extend_from_slice(&10u32.to_le_bytes()); // kind
            rec.extend_from_slice(&(i as u32).to_le_bytes()); // frame_index
        }
        for (.., atlas) in frames {
            let mut s = atlas.as_bytes().to_vec();
            s.push(0);
            rec.extend_from_slice(&0u32.to_le_bytes()); // per-frame tag
            rec.extend_from_slice(&(s.len() as u32).to_le_bytes());
            rec.extend_from_slice(&s);
        }

        let mut file = Vec::new();
        file.extend_from_slice(MAGIC);
        file.extend_from_slice(&[0u8; 8]); // version
        file.extend_from_slice(&0u32.to_le_bytes()); // size hint (unused)
        let rec_off = file.len() as u32; // 0x10
        file.extend_from_slice(&rec);
        // index: single entry
        file.extend_from_slice(&id.to_le_bytes());
        file.extend_from_slice(&rec_off.to_le_bytes());
        file.extend_from_slice(&(rec.len() as u32).to_le_bytes());
        // trailer
        file.extend_from_slice(&(id + 1).to_le_bytes());
        file
    }

    #[test]
    fn decodes_single_frame_leaf() {
        let file = build(6140, &[(0.0, 1.0, 0.0, 1.0, "BuildingP5\\zhiwu\\tree_yinxingqiu")]);
        let uv = UvListFile::read(&file).expect("decode");
        let e = uv.entries.get(&6140).expect("entry");
        assert_eq!(e.frames.len(), 1);
        assert_eq!(e.frames[0].atlas, "BuildingP5\\zhiwu\\tree_yinxingqiu");
        assert_eq!(
            (e.frames[0].u0, e.frames[0].u1, e.frames[0].v0, e.frames[0].v1),
            (0.0, 1.0, 0.0, 1.0)
        );
        assert_eq!(uv.atlas_for(6140), Some("BuildingP5\\zhiwu\\tree_yinxingqiu"));
    }

    #[test]
    fn decodes_multi_frame_animation() {
        let file = build(
            6090,
            &[
                (0.0, 1.0, 0.0, 1.0, "BuildingP5\\zhiwu\\zw_gushu_A"),
                (0.0, 1.0, 0.0, 1.0, "BuildingP5\\zhiwu\\zw_gushu_C"),
                (0.0, 1.0, 0.0, 1.0, "BuildingP5\\zhiwu\\zw_gushu_D"),
            ],
        );
        let uv = UvListFile::read(&file).expect("decode");
        let e = uv.entries.get(&6090).expect("entry");
        assert_eq!(e.frames.len(), 3);
        assert_eq!(e.frames[2].atlas, "BuildingP5\\zhiwu\\zw_gushu_D");
    }

    #[test]
    fn rejects_bad_magic() {
        let mut file = vec![0u8; 64];
        file[0..4].copy_from_slice(b"XXXX");
        assert!(matches!(UvListFile::read(&file), Err(UvListError::BadMagic)));
    }
}
