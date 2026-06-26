//! PAL5 `.ctr` grass decoder.
//!
//! Each map block ships `Map/<map>/<map>_<r>_<c>.ctr` alongside its terrain
//! (`.mp`) and objects (`.nod`). The `.ctr` describes the block's **grass** as
//! a quadtree of grass **texture layers** over the block's coarse grass grid.
//!
//! ## What the grass actually is (clean-room RE of `Pal5.exe`)
//! Grass is a **flat, grass-textured overlay on the terrain heightfield** — not
//! billboards and not a separate 3D mesh. The block carries a coarse grass grid
//! of `16×16` cells (one cell per `320`-unit terrain patch). Each quadtree leaf
//! is one **layer**: a grass texture pair (`tex0`/`tex1`, indices into the
//! `cao###` terrain-grass texture table) covering a rectangular cell range
//! `[g0,g1,g2,g3]`, with a **per-cell density** byte. The engine emits two
//! triangles per cell (over a shared `17×17` grid of grass vertices sampled
//! from the terrain) coloured by the density; multiple layers stack over the
//! same cells. Only the minority of leaves with non-grid slope detail store
//! their own vertices/triangles in-file.
//!
//! ## Container
//! ```text
//! 0x00  u32  magic  "ctr\0"  (0x00727463 LE)
//! 0x04  u32  version          = 8   (loader requires >= 7)
//! 0x08  u32  compressed_size
//! 0x0c  u32  uncompressed_size
//! 0x10  ..   zlib stream  (78 9c ...)  -> inflate to `uncompressed_size`
//! ```
//! The `version > 7` loader path (`Pal5.exe 0x6fbe30`) plain-zlib-inflates the
//! body (the codec at `0x4b8180` is zlib; embedded version string `"1.2.3"`).
//!
//! ## Inflated payload = a complete quadtree (`Pal5.exe 0x6fbfc0`)
//! A **complete quadtree of fixed depth** (depth 5 on the standard block). The
//! topology is *not* encoded inline; the reader recurses structurally.
//!
//! * Every node first consumes an **8-byte header** (two bounds floats).
//! * **Internal** node: recurse into 4 children.
//! * **Leaf** node: nine `i32` fields
//!   `[tex0, tex1, color_len, g0, g1, g2, g3, vertex_count, index_count]`,
//!   then three variable sections in order:
//!   1. `color_len / 2` bytes — the **per-cell density grid**, one byte per
//!      cell, row-major (`row = g1..=g3` outer, `col = g0..=g2` inner). The
//!      engine generates `color_len` grid triangles (2 per cell), so
//!      `color_len == 2 * cols * rows`.
//!   2. `vertex_count` × 12-byte `(x, y, z)` custom vertices (slope detail);
//!      usually `0`.
//!   3. `(index_count - color_len)` × 12-byte extra triangle records
//!      `(u16 i0, u16 i1, u16 i2, u32 color)` indexing those vertices.
//!
//! Grid triangle vertex indices are `(S+1)*row + col` (`S = 16`), referencing
//! the shared grass-vertex grid — i.e. the terrain surface at grass-grid
//! resolution; the renderer reconstructs them from the terrain heightfield.
//!
//! The remainder of the inflated buffer is MSVC `0xcd` fill (a raw memory
//! dump). The depth is auto-detected as the value whose structural parse
//! consumes every non-pad byte — verified on `kuangfengzhai_0_0.ctr`: depth 5,
//! 437 non-empty leaves, parse ends exactly at the `0xcd` boundary.

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
/// a packed color/flag word as authored. Only present on the minority of
/// leaves that carry custom (non-grid) slope geometry in-file.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct GrassTri {
    pub indices: [u16; 3],
    pub color: u32,
}

/// One leaf of the grass quadtree: a grass **texture layer** over a
/// rectangular range of the block's coarse grass grid (16×16 cells, one cell
/// per 320-unit terrain patch). See module docs.
#[derive(Debug, Clone, Serialize)]
pub struct GrassLeaf {
    /// Grass texture-set indices into the terrain texture table
    /// ([`fileformats::pal5::alp::terrain_texture_name`], the `cao###`
    /// ground-grass textures); `-1` when unused.
    pub tex0: i32,
    pub tex1: i32,
    /// Inclusive cell sub-range `[col_min, row_min, col_max, row_max]`
    /// (`g0,g1,g2,g3`) in the block's grass grid.
    pub g: [i32; 4],
    /// Per-cell density, row-major: outer `row = g1..=g3`, inner
    /// `col = g0..=g2`. `len == cols*rows == color_len/2`. Values are small
    /// coverage levels (observed 1/2/5/7, never 0). An empty vec means the
    /// leaf carries no grid layer.
    pub density: Vec<u8>,
    /// Custom in-file vertices (world-space `[x,y,z]`), present only on the
    /// minority of leaves with bespoke slope/transition geometry.
    pub vertices: Vec<[f32; 3]>,
    /// Extra in-file triangles indexing [`GrassLeaf::vertices`].
    pub triangles: Vec<GrassTri>,
}

impl GrassLeaf {
    /// Grass grid columns covered (`g2 - g0 + 1`).
    pub fn cols(&self) -> i32 {
        self.g[2] - self.g[0] + 1
    }
    /// Grass grid rows covered (`g3 - g1 + 1`).
    pub fn rows(&self) -> i32 {
        self.g[3] - self.g[1] + 1
    }
    /// Density of cell `(col, row)` in absolute grid coords, if in range.
    pub fn density_at(&self, col: i32, row: i32) -> Option<u8> {
        if self.density.is_empty()
            || col < self.g[0]
            || col > self.g[2]
            || row < self.g[1]
            || row > self.g[3]
        {
            return None;
        }
        let idx = (row - self.g[1]) * self.cols() + (col - self.g[0]);
        self.density.get(idx as usize).copied()
    }
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
        // o+12..o+28: the cell sub-range [g0, g1, g2, g3] (col_min, row_min,
        // col_max, row_max) in the block's grass grid.
        let g = [
            read_i32(self.body, o + 12),
            read_i32(self.body, o + 16),
            read_i32(self.body, o + 20),
            read_i32(self.body, o + 24),
        ];
        let vertex_count = read_i32(self.body, o + 28);
        let index_count = read_i32(self.body, o + 32);

        // Sanity-gate the counts so a wrong depth fails fast rather than
        // mallocing absurd vectors from misaligned bytes. (Only the counts
        // affect byte consumption / depth detection; `g` is captured as-is,
        // since empty/edge leaves may carry uninitialized grid bounds.)
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

        // 1) Per-cell density grid: `color_len / 2` bytes, one per cell
        //    (the engine generates 2 grid triangles per cell, so
        //    `color_len == 2 * cols * rows`).
        let mut density = Vec::new();
        if color_len > 0 {
            let Some(co) = self.need(color_len / 2) else {
                return;
            };
            density.extend_from_slice(&self.body[co..co + color_len / 2]);
        }

        // 2) Custom in-file vertices (world-space), present only on leaves
        //    with bespoke slope/transition geometry.
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

        // 3) Extra (non-grid) triangle records: `index_count - color_len`
        //    of them, indexing the custom vertices above.
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

        // Keep every leaf that carries a grass layer (a density grid) or
        // custom geometry. Pure-grid leaves (the majority) have a density
        // grid but no in-file vertices.
        if !density.is_empty() || !vertices.is_empty() {
            self.leaves.push(GrassLeaf {
                tex0,
                tex1,
                g,
                density,
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

    /// Append a leaf with a grid layer (`g` range + per-cell density) and
    /// optional custom vertices/extra-triangles.
    fn push_leaf(
        body: &mut Vec<u8>,
        tex0: i32,
        tex1: i32,
        g: [i32; 4],
        density: &[u8],
        verts: &[[f32; 3]],
        tris: &[[u16; 3]],
    ) {
        push_header(body);
        // color_len = 2 * cell count; index_count = color_len + extra tris.
        let color_len = (density.len() * 2) as i32;
        let index_count = color_len + tris.len() as i32;
        for v in [
            tex0,
            tex1,
            color_len,
            g[0],
            g[1],
            g[2],
            g[3],
            verts.len() as i32,
            index_count,
        ] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        // density grid: color_len/2 bytes.
        body.extend_from_slice(density);
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
        // Complete depth-1 quadtree: root internal + four leaves. One leaf is
        // a pure 2×1-cell grass-grid layer; one carries custom geometry.
        let mut body = Vec::new();
        push_header(&mut body); // root (internal)
        // Leaf 0: grid layer, cols g0..g2 = 0..1 (2 cols), rows g1..g3 = 0..0
        // (1 row) → 2 cells, density [3, 5].
        push_leaf(&mut body, 10, 7, [0, 0, 1, 0], &[3, 5], &[], &[]);
        // Leaf 1: custom slope geometry (no grid), one triangle.
        push_leaf(
            &mut body,
            2,
            4,
            [0, 0, 0, 0],
            &[],
            &[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]],
            &[[0, 1, 2]],
        );
        push_leaf(&mut body, -1, -1, [0, 0, 0, 0], &[], &[], &[]);
        push_leaf(&mut body, -1, -1, [0, 0, 0, 0], &[], &[], &[]);
        // Trailing memory-fill padding (as the real dumps carry).
        body.extend(std::iter::repeat(PAD).take(64));

        let file = wrap(&body);
        let ctr = CtrFile::read(&file).expect("decode");
        assert_eq!(ctr.version, 8);
        assert_eq!(ctr.depth, 1);
        // The two non-empty leaves are kept (the two empty ones dropped).
        assert_eq!(ctr.leaves.len(), 2);

        let grid = &ctr.leaves[0];
        assert_eq!((grid.tex0, grid.tex1), (10, 7));
        assert_eq!(grid.g, [0, 0, 1, 0]);
        assert_eq!(grid.density, vec![3, 5]);
        assert_eq!((grid.cols(), grid.rows()), (2, 1));
        assert_eq!(grid.density_at(0, 0), Some(3));
        assert_eq!(grid.density_at(1, 0), Some(5));
        assert_eq!(grid.density_at(2, 0), None);
        assert!(grid.vertices.is_empty());

        let custom = &ctr.leaves[1];
        assert_eq!(custom.vertices.len(), 3);
        assert_eq!(custom.vertices[1], [4.0, 5.0, 6.0]);
        assert_eq!(custom.triangles.len(), 1);
        assert_eq!(custom.triangles[0].indices, [0, 1, 2]);
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
        assert!(!ctr.leaves.is_empty(), "block should carry grass layers");
        // Most leaves are pure grid layers (density, no in-file vertices).
        assert!(ctr.leaves.iter().any(|l| !l.density.is_empty()));
        for leaf in &ctr.leaves {
            // Grid cell range is sane (block grass grid is 16×16).
            for &v in &leaf.g {
                assert!((0..=64).contains(&v));
            }
            // density length matches the cell count.
            if !leaf.density.is_empty() {
                assert_eq!(leaf.density.len() as i32, leaf.cols() * leaf.rows());
            }
            // Custom vertices, when present, lie within a sane world band.
            for v in &leaf.vertices {
                assert!(v[0].is_finite() && v[1].is_finite() && v[2].is_finite());
                assert!((-2000.0..=8000.0).contains(&v[0]));
                assert!((-3000.0..=6000.0).contains(&v[1]));
                assert!((-2000.0..=8000.0).contains(&v[2]));
            }
            for t in &leaf.triangles {
                for &i in &t.indices {
                    assert!((i as usize) < leaf.vertices.len());
                }
            }
        }
    }
}
