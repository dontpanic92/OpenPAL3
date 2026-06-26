# PAL5 `.ctr` grass — reverse-engineering findings

Clean-room RE of `F:\PAL5\Pal5.exe.unpacked.exe` (ImageBase `0x400000`).
Sample data: `Map/kuangfengzhai/kuangfengzhai_0_0.ctr` (extracted from `Map.pkg`,
key `Y%H^uz6i`; 89 159 bytes on disk, inflates to 1 693 528 bytes).

This document records what is **established** vs **still unknown**, so the
renderer can be driven by facts instead of guesses. The current yaobow renderer
(`yaobow/shared/src/openpal5/grass.rs`) draws the **wrong** triangle class (see
§6) and therefore produces tall green curtains / a sky "cone"; see §8.

---

## 1. Container (established)

```
0x00  u32  magic            "ctr\0"  (0x00727463 LE)
0x04  u32  version          = 8      (loader requires >= 7)
0x08  u32  compressed_size
0x0c  u32  uncompressed_size
0x10  ..   zlib stream (78 9c ...)   -> inflate to uncompressed_size
```

The `version > 7` path (`0x6fbe30`) plain-zlib-inflates the body (codec
`0x4b8180`, embedded `"1.2.3"`). Loader header reader `0x77b390`
(magic table `0x977514`). Block loader `0x784200` builds `"%s_%d_%d.ctr"`.

The inflated body is a **raw memory dump**: real data first, then MSVC `0xcd`
uninitialised fill to the end. The real-data boundary is the last non-`0xcd`
byte (word-aligned).

## 2. Quadtree (established — loader `0x6fbfc0`)

A **complete quadtree of fixed depth** (depth 5 on the standard block →
`4^5 = 1024` leaf slots; 437 carry data on the sample). Topology is *not* stored
inline; the reader recurses structurally.

* Every node consumes an **8-byte header** first (two `float` bounds — used by
  the draw/visit code as part of an AABB, see §5).
* Node `+0x2c` is the child-array pointer; internal nodes recurse 4 children
  (stride `0x5c` = 92 bytes per node struct).
* Leaf node (when `[node+0x2c] == 0`): reads the leaf record (§3).

Auto-detecting depth by "the parse that consumes every non-pad byte" is what
yaobow's `fileformats::pal5::ctr` does, and it lands exactly on the `0xcd`
boundary for the sample (depth 5, 437 leaves).

## 3. Leaf record (established — `0x6fbfc0` continued)

Nine `i32` header fields, then three variable sections, in order:

```
i32 tex0          (node+0x10 region; see §7 — a GRID COORD, not a texture id)
i32 tex1          (see §7)
i32 color_len     -> number of grid triangles generated (= 2*cols*rows)
i32 g0  (col_min) ] inclusive cell sub-range in the block's 16x16 grass grid
i32 g1  (row_min) ]
i32 g2  (col_max) ]
i32 g3  (row_max) ]
i32 vertex_count  -> custom xyz vertices (field +0x54, buffer +0x50)
i32 index_count   -> total triangles = grid (color_len) + custom; field +0x48,
                     buffer +0x44

section 1: color_len/2 density bytes  (one per cell, row-major g1..=g3 x g0..=g2)
section 2: vertex_count * 12 bytes    (custom xyz, 3x f32)
section 3: (index_count - color_len) * 12 bytes  custom triangle records
```

**Triangle record = 12 bytes** (write code `0x6fc385..0x6fc3f5`,
`0x6fd048..`):

```
u16 i0
u16 i1
u16 i2
u16 (padding, unused)
u32 color/flags
```

yaobow's decoder reads indices at +0,+2,+4 and color at +8 — **verified
correct** against the loader.

### Grid triangles are generated, not stored (established `0x6fc2aa..0x6fc499`)

For each cell in `g1..=g3 x g0..=g2` the loader emits **2 triangles** into the
`+0x44` buffer, indices into a **lattice** of `(S+1)*(S+1)` vertices
(`S = [singleton 0xa85790 + 4]`, `S = 16` on the sample):

```
A = (S+1)*row + col
B = (S+1)*(row+1) + col
C = A + 1
D = B + 1
tris: (A,B,C) and (C,B,D)         [exact winding per disasm]
color = (densityByte << 12) | 1   <-- bit0 = 1
```

So `color_len == 2 * cols * rows`, and grid-triangle colours always have
**bit0 set** and density in bits 12-19.

The custom (section-3) triangles keep their stored colour. On the sample these
are `0x00ff0002`, `0x00ff0004`, `0x00030002`, `0x00050002`, `0x00070002`,
`0x00090002` — all with **bit0 = 0** and **bit1 = 1**.

## 4. Two vertex buffers (established — draw/visit `0x6fce70`)

The draw/visit routine iterates the single `+0x44` triangle buffer
(`count = [node+0x4c]`) and, per triangle, selects the vertex buffer by
**colour bit0** (`0x6fd0ab: and ecx,1; je custom`):

* **bit0 = 1 (grid triangle):** index a **44-byte-stride** (`0x2c`) vertex
  buffer, base `0x6ff330(node+0x30)` (returns `[obj+4]`). `node+0x30` is built
  by `0x771dc0(tex0,tex1)` (§7). This buffer is the **upright grass-blade
  lattice** — it must carry position + UVs (the engine never synthesises UVs
  for these).
* **bit0 = 0 (custom triangle):** index the **12-byte-stride** (`0xc`) xyz
  buffer at `node+0x50` (the section-2 custom vertices). These are the tall
  "curtain" verts.

**This split is the crux:** the visible short grass is almost certainly the
**grid/lattice** class (44-byte, has UVs, density-driven), while the **custom
12-byte** class is a separate thing (tall curtains — likely bounding/occlusion
shells, see §6/§8). yaobow currently renders **only** the custom class.

## 5. Colour-flag filtering (established — `0x6fce70`)

Per triangle, before drawing/visiting:

```
c = color[tri]
if (c & 0x80000000) != 0            -> skip          (0x6fcffe)  bit31 = cull
if (c & pass_mask) == 0             -> skip          (0x6fd01b)  pass_mask = arg [ebp+0x14]
if global[0x9dfbc0]==0 && (c & 0x0ff00000)==0 -> skip (0x6fd03e)
... draw ...
density = (c & 0x000ff000) >> 12                     (0x6fd142)  -> global 0x9dfbc4
if density & 0x80: density = -1
```

So `pass_mask` (a caller argument) gates which triangle classes render in a
given pass, and bits 12-19 carry the per-triangle density used for shading.

**ALL callers of the visitors are PICK / query passes — none is the visual
draw** (confirmed this session):

* Three near-identical visitor instantiations exist: `0x6fce70`, `0x6fd2a0`,
  `0x6fd5b0` (same grid/custom split + colour filter, different callback).
* Callers pass ray-pick callbacks and return a nearest-hit point + distance:
  * `0x7716e7` / `0x77188f` → callback `0x772950`, masks `1` / `7`.
  * `0x771993` / `0x771a7d` / `0x771b28` → callback `0x773380`.
* `0x772950` **disassembled = ray–triangle intersection**: it dots the triangle
  normal (`tri+0x4c`) with a direction (`arg+0xc`), back-face/threshold tests,
  then edge-vector plane math. Definitively picking, not GPU emit.

**Conclusion: the `.ctr` quadtree + custom triangles are a
COLLISION / PICK / spatial-query structure, NOT the visible grass mesh.** This
is why every attempt to render the custom (bit0=0) triangles looks wrong — they
are grass *collision hulls* (the tall 461-unit "curtains" are the hull shells;
the shorter custom tris are finer near-ground hull detail).

The visible grass blades are the **grid/lattice** class (44-byte buffer, §4),
built per block and indexed by `tex0/tex1`. Its GPU draw path is a separate
system not yet fully located (§9), but the lattice is driven by the **density
grid** (`(density<<12)|1` grid triangles) — i.e. the density grid is the
authoritative grass coverage map.

The world→patch helpers around `0x700f00` (`(worldXZ - origin)/cellSize →
patch[tex0+tex1*width]`) and the patch iterator at `0x700f70` confirm a
block-wide patch collection (≤256 entries) keyed by grid coords.

## 6. What the data actually contains (established — data dump)

`leaf #126` (`tex0=1 tex1=14 g=[0,8,7,15] verts=82 tris=85 density_cells=64`):

* **verts 0..29**: every vertex sits on one of exactly **two Y planes** —
  `Y = 601.8007` (bottom) or `Y = 1063.4725` (top), same XZ pairs. tris 0..25
  weave them into a continuous **vertical curtain ~461 units tall** that winds
  through XZ (max edge ≈ 463).
* **verts 30..81**: varied heights (`Y ≈ 597..753`), tris 26.. with max edge
  ≈ 115..153 — plausibly **real short blades** (~150 tall).

Across the block: 258/437 leaves carry custom geometry (8 833 verts, 9 507
tris); **105 leaves have a custom-vertex Y-span > 250** (up to 474). The global
custom-vertex bbox is `min=[87.4, 474.7, 2353.4] max=[4917.9, 1162.7, 5122.6]`
— i.e. all within the block, Y within the terrain range.

There is **no single shared apex in the data** (most-referenced vertex cell:
9 refs). The "cone" yaobow renders is therefore **not** in the file (see §8).

The uniform two-plane structure of the tall curtains (flat `601.8` floor, flat
`1063.5` ceiling) strongly suggests they are a **bounding / occlusion / density
hull for the leaf**, not blade geometry — consistent with them being a separate
colour class drawn (if at all) in a non-visible pass.

## 7. `tex0` / `tex1` (partly established)

`0x771dc0(tex0, tex1)`:

```
width = 0x707bd0(singleton 0x6dc640)
elem  = 0x704600(singleton, tex0 + tex1*width)   -> stored at node+0x30
```

So `tex0`/`tex1` are **2D grid coordinates** indexing a block-wide array of
grass-patch objects (`idx = tex0 + tex1*width`), **not** `cao###` texture names.
The patch object's `+4` is the 44-byte lattice vertex buffer (§4).

> Earlier yaobow notes treated `tex0/tex1` as `cao###` terrain-texture indices.
> That is **likely wrong** for the grass-blade lattice. (The `cao###` textures
> are 512² DXT5 *ground* textures whose alpha is noise/detail, range 0..171 —
> **not** a blade alpha-cutout. Verified by decoding `cao011.dds`.) The blade
> texture source is unknown (§9).

## 8. Why the current yaobow render is wrong (established)

`grass.rs` renders the **custom 12-byte triangles** (the bit0=0 class) as solid
alpha-test quads with synthesized UVs. Consequences seen on screen:

1. **Tall curtains** — it draws the 461-unit hull walls (verts 0..29) as opaque
   grass → green slabs climbing into the sky.
2. **The "cone"** — a height *clamp* using the terrain heightfield
   (`build_block_grass_heights`) returns **NaN** for grass-grid corners the
   `.mp` patches don't cover; a NaN vertex Y collapses every triangle that
   touches it to a single clip-space point → the shared apex / cone the user
   sees. (The cone is a **rendering artifact of the clamp**, not data, which is
   why no cap/clamp fixes it — confirmed: data has no shared apex.)
3. **Opaque sheet** — the procedural blade texture is too dense and the custom
   class isn't the see-through blade lattice, so overlapping hulls read as a
   solid sheet.

## 9. Open unknowns (to resolve before the next render attempt)

> **Resolved this session:** the `.ctr` triangle buffers are picking/collision
> geometry (§5). yaobow must **stop rendering the custom (bit0=0) triangles** —
> they are collision hulls and are the sole source of the tall curtains / sky
> "cone". The visible grass is the density-grid-driven lattice.

1. **The visual draw path for the lattice.** Not yet located. The patch
   collection (≤256 entries, keyed `tex0+tex1*width`) holds the 44-byte lattice
   buffers; find the per-block builder that fills them from the density grid +
   terrain, and the per-frame draw (likely `DrawIndexedPrimitive` over the
   lattice). Search around the patch system (`0x700000`–`0x701000`) and the
   grass class that owns the `.ctr` loader (`0x77b43d`, `0x786c97`, `0x80fe8c`
   call `0x6fbe30`).
2. **The 44-byte lattice vertex layout** (pos / normal / uv / colour) once the
   builder is found — this is the geometry to render.
3. **Blade texture source.** Not `cao###` (those are 512² DXT5 *ground*
   textures; `cao011.dds` alpha is detail noise 0..171, not a blade cutout).
   Find the grass-blade alpha-cutout atlas under `Texture\...`.
4. **Wind animation** — per-frame tip-vertex offset in the draw path.
5. **`S` / grid sizing** for non-standard blocks (sample `S=16`, 16×16 cells).

## 10. RE tooling (session)

Scripts in the session `files/` dir:
* `rectr.py <va>...`     — capstone disassembler (ImageBase 0x400000).
* `callxref.py <va>`     — E8 rel32 call xrefs to a target.
* `xref.py <va>`         — data xrefs.
* `findgrassdraw2.py`    — heuristic scan for grass draw/visit candidates.

Data dumps: `cargo run -p fileformats --example ctr_dump -- <file.ctr> [leaf]`.
Extract from pkg: `cargo run -p packfs --example find_extract -- <pkg> <needle> [out]`.
Decode dds: `cargo run -p shared --example dds_view -- <file.dds> <out_base>`.

Key addresses:
| addr | role |
|------|------|
| `0x6fbfc0` | leaf/quadtree loader |
| `0x6fbe30` | container/zlib loader (v>7) |
| `0x6fce70` | grass triangle visitor (grid vs custom split, colour filter) |
| `0x6fccf0` | sibling visitor (recursion peer) |
| `0x6ff330` | `node+0x30 -> [obj+4]` (44-byte lattice base) |
| `0x771dc0` | `node+0x30 = patchArray[tex0 + tex1*width]` |
| `0x772950` | triangle–ray pick callback |
| `0x7716xx`/`0x7718xx` | pick traversals (mask 1 / 7) |
