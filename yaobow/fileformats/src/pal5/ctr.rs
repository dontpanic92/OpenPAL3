//! PAL5 `.ctr` grass decoder.
//!
//! Each map block ships `Map/<map>/<map>_<r>_<c>.ctr` alongside its terrain
//! (`.mp`) and objects (`.nod`). The `.ctr` holds the block's **grass** as a
//! pre-built quadtree of small grass meshes.
//!
//! ## Container (clean-room RE of `Pal5.exe`)
//! ```text
//! 0x00  u32  magic  "ctr\0"  (0x00727463 LE)
//! 0x04  u32  version          = 8   (loader requires >= 7)
//! 0x08  u32  compressed_size
//! 0x0c  u32  uncompressed_size
//! 0x10  ..   zlib stream  (78 9c ...)  -> inflate to `uncompressed_size`
//! ```
//! Mirrors the `.mp` decoder: locate the `78 9c` zlib signature and inflate
//! with `miniz_oxide`. The `version > 7` loader path (`Pal5.exe 0x6fbe30`)
//! plain-zlib-inflates the body (the codec at `0x4b8180` is zlib itself —
//! its embedded version string is `"1.2.3"`).
//!
//! ## Inflated payload = a complete quadtree (`Pal5.exe 0x6fbfc0`)
//! The body is a **complete quadtree of fixed depth** (depth 5 → 1024 leaves,
//! a 32×32 leaf grid for the standard 5120-unit block). The topology is
//! *not* encoded inline — every internal node has exactly four children down
//! to the fixed depth — so the reader recurses structurally. Nodes are read
//! sequentially via a single cursor:
//!
//! * Every node (internal or leaf) first consumes an **8-byte header**
//!   (two floats; a node-bounds hint the geometry doesn't need).
//! * **Internal** node (`depth < max_depth`): recurse into 4 children.
//! * **Leaf** node (`depth == max_depth`): then reads nine `i32`/`f32`
//!   fields — `[tex0, tex1, color_len, g0, g1, g2, g3, vertex_count,
//!   index_count]` — followed by three variable sections:
//!   1. `color_len / 2` bytes of packed per-cell color source (skipped);
//!   2. `vertex_count` × 12-byte `(x, y, z)` **world-space grass vertices**;
//!   3. `(index_count - color_len)` × 12-byte **triangle** records
//!      `(u16 i0, u16 i1, u16 i2, u32 color)` indexing those vertices.
//!
//! The remainder of the inflated buffer is MSVC `0xcd` uninitialized-memory
//! fill (the file is a raw memory dump; ~84% padding, which zlib collapses).
//! The depth is auto-detected by the unique value that consumes all non-pad
//! bytes — verified on `kuangfengzhai_0_0.ctr`: depth 5, 1024 leaves,
//! 8833 grass vertices, parse ends exactly at the `0xcd` boundary.
//!
//! `tex0`/`tex1` are indices into the map's grass texture set
//! (`Texture\grass\…`); texture/UV resolution is a renderer concern.

use serde::Serialize;

/// Header magic: ASCII `"ctr\0"` little-endian.
const MAGIC: u32 = 0x0072_7463;

/// MSVC uninitialized-memory fill byte; trails the real data in the dump.
const PAD: u8 = 0xcd;

/// Highest quadtree depth probed during auto-detection (depth 5 is the
/// observed value; a few extra give headroom without risking false hits).
const MAX_PROBE_DEPTH: usize = 8;

#[derive(thiserror::Error, Debug)]
pub enum CtrError {
    #[error("file too small")]
    TooSmall,
    #[error("not a .ctr file (bad magic {0:#x})")]
    BadMagic(u32),
    #[error("unsupported version {0} (< 7)")]
    UnsupportedVersion(u32),
    #[error("zlib stream not found")]
    NoZlib,
    #[error("decompression failed: {0}")]
    Inflate(String),
    #[error("could not determine quadtree depth (corrupt grass payload)")]
    BadTree,
}

/// One grass triangle: three indices into the owning leaf's `vertices`, plus
/// a packed RGBA-ish color/flag word as authored.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GrassTri {
    pub indices: [u16; 3],
    pub color: u32,
}

/// One leaf of the grass quadtree: a small indexed mesh of grass blades.
#[derive(Debug, Clone, Serialize)]
pub struct GrassLeaf {
    /// Grass texture-set indices (`Texture\grass\…`); `-1` when unused.
    pub tex0: i32,
    pub tex1: i32,
    /// World-space grass vertices `[x, y, z]`.
    pub vertices: Vec<[f32; 3]>,
    /// Triangles indexing [`GrassLeaf::vertices`].
    pub triangles: Vec<GrassTri>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CtrFile {
    pub version: u32,
    /// Detected complete-quadtree depth (leaves = `4.pow(depth)`).
    pub depth: usize,
    /// Only non-empty leaves (those carrying at least one vertex) are kept.
    pub leaves: Vec<GrassLeaf>,
}

impl CtrFile {
    /// Decode a raw `.ctr` file (container header + zlib body + quadtree).
    pub fn read(raw: &[u8]) -> Result<CtrFile, CtrError> {
        if raw.len() < 0x10 {
            return Err(CtrError::TooSmall);
        }
        let magic = read_u32(raw, 0);
        if magic != MAGIC {
            return Err(CtrError::BadMagic(magic));
        }
        let version = read_u32(raw, 4);
        if version < 7 {
            return Err(CtrError::UnsupportedVersion(version));
        }

        // Inflate the zlib body. Locate by signature so small header
        // variations are tolerated; the canonical offset is 0x10.
        let zpos = raw
            .windows(2)
            .position(|w| w == [0x78, 0x9c])
            .ok_or(CtrError::NoZlib)?;
        let body = miniz_oxide::inflate::decompress_to_vec_zlib(&raw[zpos..])
            .map_err(|e| CtrError::Inflate(format!("{:?}", e)))?;

        let (depth, leaves) = parse_tree(&body).ok_or(CtrError::BadTree)?;
        Ok(CtrFile {
            version,
            depth,
            leaves,
        })
    }

    /// Total grass vertices across all leaves.
    pub fn vertex_count(&self) -> usize {
        self.leaves.iter().map(|l| l.vertices.len()).sum()
    }
}

/// Detect the complete-quadtree depth and parse the leaves.
///
/// The engine uses a fixed depth baked into the grass object; we recover it
/// by choosing the unique depth whose structural parse consumes every
/// non-padding byte (the trailing `0xcd` memory fill is the boundary). This
/// is robust to maps with a different leaf-grid size than the common 32×32.
fn parse_tree(body: &[u8]) -> Option<(usize, Vec<GrassLeaf>)> {
    // The real data ends at the last non-`0xcd` byte; everything after is
    // uninitialized fill the writer dumped verbatim.
    let data_end = body.iter().rposition(|&b| b != PAD).map(|p| p + 1)?;

    for depth in 1..=MAX_PROBE_DEPTH {
        let mut p = TreeParser {
            body,
            cur: 0,
            leaves: Vec::new(),
            ok: true,
        };
        p.node(0, depth);
        if p.ok && align4(p.cur) >= data_end && p.cur <= body.len() {
            return Some((depth, p.leaves));
        }
    }
    None
}

/// Round a byte count up to the next 4-byte boundary (the writer pads the
/// data region to a word before the `0xcd` fill).
fn align4(n: usize) -> usize {
    (n + 3) & !3
}

struct TreeParser<'a> {
    body: &'a [u8],
    cur: usize,
    leaves: Vec<GrassLeaf>,
    ok: bool,
}

impl<'a> TreeParser<'a> {
    fn need(&mut self, n: usize) -> Option<usize> {
        if !self.ok || self.cur + n > self.body.len() {
            self.ok = false;
            return None;
        }
        let o = self.cur;
        self.cur += n;
        Some(o)
    }

    fn node(&mut self, depth: usize, max_depth: usize) {
        // Every node carries an 8-byte header (two bounds floats) first.
        if self.need(8).is_none() {
            return;
        }
        if depth < max_depth {
            for _ in 0..4 {
                self.node(depth + 1, max_depth);
                if !self.ok {
                    return;
                }
            }
            return;
        }
        self.leaf();
    }

    fn leaf(&mut self) {
        let Some(o) = self.need(36) else { return };
        let tex0 = read_i32(self.body, o);
        let tex1 = read_i32(self.body, o + 4);
        let color_len = read_i32(self.body, o + 8);
        // o+12..o+28: four grid-bound ints (cell sub-range; unused here).
        let vertex_count = read_i32(self.body, o + 28);
        let index_count = read_i32(self.body, o + 32);

        // Sanity-gate the counts so a wrong depth fails fast rather than
        // mallocing absurd vectors from misaligned bytes.
        if !(0..=1 << 16).contains(&color_len)
            || !(0..=1 << 20).contains(&vertex_count)
            || !(0..=1 << 20).contains(&index_count)
        {
            self.ok = false;
            return;
        }
        let (color_len, vertex_count, index_count) = (
            color_len as usize,
            vertex_count as usize,
            index_count as usize,
        );

        // 1) Packed per-cell color source: `color_len / 2` bytes, skipped.
        if color_len > 0 && self.need(color_len / 2).is_none() {
            return;
        }

        // 2) World-space grass vertices.
        let mut vertices = Vec::with_capacity(vertex_count);
        if vertex_count > 0 {
            let Some(vo) = self.need(vertex_count * 12) else {
                return;
            };
            for k in 0..vertex_count {
                let b = vo + k * 12;
                vertices.push([
                    read_f32(self.body, b),
                    read_f32(self.body, b + 4),
                    read_f32(self.body, b + 8),
                ]);
            }
        }

        // 3) Triangle records (only the tail beyond `color_len` is streamed).
        let mut triangles = Vec::new();
        if index_count > color_len {
            let tri_count = index_count - color_len;
            let Some(to) = self.need(tri_count * 12) else {
                return;
            };
            for k in 0..tri_count {
                let b = to + k * 12;
                triangles.push(GrassTri {
                    indices: [
                        read_u16(self.body, b),
                        read_u16(self.body, b + 2),
                        read_u16(self.body, b + 4),
                    ],
                    color: read_u32(self.body, b + 8),
                });
            }
        }

        // Keep only leaves that actually carry grass.
        if !vertices.is_empty() {
            self.leaves.push(GrassLeaf {
                tex0,
                tex1,
                vertices,
                triangles,
            });
        }
    }
}

fn read_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn read_i32(b: &[u8], o: usize) -> i32 {
    read_u32(b, o) as i32
}
fn read_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn read_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Append a node header (two bounds floats) to the body.
    fn push_header(body: &mut Vec<u8>) {
        body.extend_from_slice(&0.0f32.to_le_bytes());
        body.extend_from_slice(&0.0f32.to_le_bytes());
    }

    /// Append a leaf: nine i32/f32 fields + optional vertices + triangles.
    /// `color_len` is set to 0 so every streamed index record is a triangle.
    fn push_leaf(body: &mut Vec<u8>, tex0: i32, tex1: i32, verts: &[[f32; 3]], tris: &[[u16; 3]]) {
        push_header(body);
        for v in [
            tex0,
            tex1,
            0, // color_len
            0, // grid g0..g3
            0,
            0,
            0,
            verts.len() as i32, // vertex_count
            tris.len() as i32,  // index_count (== triangle count, color_len = 0)
        ] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        for v in verts {
            for c in v {
                body.extend_from_slice(&c.to_le_bytes());
            }
        }
        for t in tris {
            for i in t {
                body.extend_from_slice(&i.to_le_bytes());
            }
            body.extend_from_slice(&0u16.to_le_bytes()); // 2-byte gap (record is 12 bytes)
            body.extend_from_slice(&0x00ff_0002u32.to_le_bytes());
        }
    }

    /// Wrap an inflated body as a `.ctr` container (header + zlib).
    fn wrap(body: &[u8]) -> Vec<u8> {
        let zlib = miniz_oxide::deflate::compress_to_vec_zlib(body, 6);
        let mut file = Vec::new();
        file.extend_from_slice(&MAGIC.to_le_bytes());
        file.extend_from_slice(&8u32.to_le_bytes()); // version
        file.extend_from_slice(&(zlib.len() as u32).to_le_bytes());
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend_from_slice(&zlib);
        file
    }

    #[test]
    fn decodes_depth1_quadtree() {
        // Complete depth-1 quadtree: root internal + four leaves; one leaf
        // carries a single grass triangle.
        let mut body = Vec::new();
        push_header(&mut body); // root (internal)
        push_leaf(
            &mut body,
            10,
            7,
            &[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            &[[0, 1, 2]],
        );
        push_leaf(&mut body, -1, -1, &[], &[]);
        push_leaf(&mut body, -1, -1, &[], &[]);
        push_leaf(&mut body, -1, -1, &[], &[]);
        // Trailing memory-fill padding (as the real dumps carry).
        body.extend(std::iter::repeat(PAD).take(64));

        let file = wrap(&body);
        let ctr = CtrFile::read(&file).expect("decode");
        assert_eq!(ctr.version, 8);
        assert_eq!(ctr.depth, 1);
        // Only the non-empty leaf is kept.
        assert_eq!(ctr.leaves.len(), 1);
        let leaf = &ctr.leaves[0];
        assert_eq!((leaf.tex0, leaf.tex1), (10, 7));
        assert_eq!(leaf.vertices.len(), 3);
        assert_eq!(leaf.vertices[1], [4.0, 5.0, 6.0]);
        assert_eq!(leaf.triangles.len(), 1);
        assert_eq!(leaf.triangles[0].indices, [0, 1, 2]);
        assert_eq!(ctr.vertex_count(), 3);
    }

    #[test]
    fn rejects_bad_magic() {
        let file = [0u8; 32];
        assert!(matches!(CtrFile::read(&file), Err(CtrError::BadMagic(_))));
    }

    /// Decode a real shipped block when the PAL5 assets are present. Skipped
    /// (not failed) when the unencrypted `.ctr` fixture isn't available, so
    /// CI without game data stays green. To produce the fixture, extract
    /// `Map/<map>/<map>_0_0.ctr` from `Map.pkg`.
    #[test]
    fn decodes_real_block_if_present() {
        let path = std::env::var("YAOBOW_PAL5_CTR")
            .unwrap_or_else(|_| "/tmp/kuangfengzhai_0_0.ctr".to_string());
        let Ok(raw) = std::fs::read(&path) else {
            eprintln!("skipping: no .ctr fixture at {path}");
            return;
        };
        let ctr = CtrFile::read(&raw).expect("decode real block");
        assert_eq!(ctr.depth, 5, "standard PAL5 block is a depth-5 quadtree");
        assert!(ctr.vertex_count() > 0, "block should carry grass");
        // Every vertex must lie within a sane world band for block (0,0).
        for leaf in &ctr.leaves {
            for v in &leaf.vertices {
                assert!(v[0].is_finite() && v[1].is_finite() && v[2].is_finite());
                assert!((-2000.0..=8000.0).contains(&v[0]));
                assert!((-3000.0..=6000.0).contains(&v[1]));
                assert!((-2000.0..=8000.0).contains(&v[2]));
            }
            // Triangle indices must reference existing vertices.
            for t in &leaf.triangles {
                for &i in &t.indices {
                    assert!((i as usize) < leaf.vertices.len());
                }
            }
        }
    }
}
