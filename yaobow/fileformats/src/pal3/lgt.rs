//! PAL3 `<index>.lgt` — dynamic scene light table.
//!
//! Layout (little-endian), fully reverse-engineered against all 533 PAL3
//! scenes (see `generated/pal3_scn.md`):
//!
//! ```text
//! 0x00  u32              light_count
//! repeat light_count × 148 bytes:
//!   +0x00  f32[16]       4x4 transform (row-major); world position = row 3
//!                        (floats at +0x30,+0x34,+0x38); m33 @ +0x3C == 1.0
//!   +0x40  f32           unknown, always 0.0
//!   +0x44  f32[3]        RGB color (dim fill ~0.1, bright key up to ~1.2)
//!   +0x64  u32           type  (always 1 = omni/point)
//!   +0x68  u32           flag  (always 1 = enabled)
//!   +0x74  f32           range1 (FLT_MAX ⇒ no attenuation)
//!   +0x78  f32           range2 (FLT_MAX)
//!   +0x7C  f32           cone  (0.0 for omni)
//!   +0x88  f32[3]        direction ((-1,-1,-1) sentinel for omni)
//! ```
//!
//! Every shipped light is `type == 1`, `flag == 1`, `range == FLT_MAX`, so the
//! engine treats them as un-attenuated omni point lights positioned in world
//! space (distant lights act nearly directional).

use std::io::{Read, Seek};

use binrw::{BinRead, BinResult};
use serde::Serialize;

/// One light source (148 bytes on disk).
#[derive(Debug, Clone, BinRead, Serialize)]
#[brw(little)]
pub struct Light {
    /// Row-major 4x4 transform. The world position is the translation row
    /// (`transform[12..15]`); see [`Light::position`].
    pub transform: [f32; 16],

    /// Always `0.0` in the shipped corpus (purpose unconfirmed).
    pub _unknown_40: f32,

    /// RGB color / intensity (pre-scaled; the bright overhead "key" lights
    /// carry values > 1.0).
    pub color: [f32; 3],

    /// Padding between the color and the type tag (always zero).
    pub _pad_50: [u32; 5],

    /// Light type tag. `1` (omni/point) for every shipped light.
    pub light_type: u32,

    /// Enabled flag. `1` for every shipped light.
    pub flag: u32,

    /// Reserved dwords between `flag` and the range pair (always zero).
    pub _pad_6c: [u32; 2],

    /// Inner / outer range. Both are `FLT_MAX` in the shipped corpus
    /// (i.e. no distance attenuation).
    pub range: [f32; 2],

    /// Spot cone angle. `0.0` for omni lights.
    pub cone: f32,

    /// Trailing parameters (a direction vector at `+0x88` plus reserved
    /// space) kept opaque; unused for omni point lights.
    pub _tail: [f32; 5],
}

impl Light {
    /// World-space position (translation row of [`Light::transform`]).
    pub fn position(&self) -> [f32; 3] {
        [self.transform[12], self.transform[13], self.transform[14]]
    }
}

/// A parsed `.lgt` file: an ordered list of scene lights.
#[derive(Debug, Clone, BinRead, Serialize)]
#[brw(little)]
pub struct LgtFile {
    pub light_count: u32,

    #[br(count = light_count)]
    pub lights: Vec<Light>,
}

/// Parse a `.lgt` light table from a reader.
pub fn read_lgt(reader: &mut (impl Read + Seek)) -> BinResult<LgtFile> {
    LgtFile::read(reader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Build a single-light `.lgt` buffer with an identity orientation, a
    /// known translation, and a known color, then verify the decode.
    #[test]
    fn parses_single_light() {
        let mut buf: Vec<u8> = vec![];
        buf.extend_from_slice(&1u32.to_le_bytes()); // light_count

        // 4x4 row-major transform; translation row holds the position.
        let mut m = [0f32; 16];
        m[0] = 1.0;
        m[5] = 1.0;
        m[10] = 1.0;
        m[12] = 376.605; // x
        m[13] = 977.196; // y
        m[14] = -2223.708; // z
        m[15] = 1.0;
        for v in m {
            buf.extend_from_slice(&v.to_le_bytes());
        }

        buf.extend_from_slice(&0f32.to_le_bytes()); // _unknown_40
        for c in [0.129f32, 0.134, 0.136] {
            buf.extend_from_slice(&c.to_le_bytes()); // color
        }
        for _ in 0..5 {
            buf.extend_from_slice(&0u32.to_le_bytes()); // _pad_50
        }
        buf.extend_from_slice(&1u32.to_le_bytes()); // light_type
        buf.extend_from_slice(&1u32.to_le_bytes()); // flag
        for _ in 0..2 {
            buf.extend_from_slice(&0u32.to_le_bytes()); // _pad_6c
        }
        buf.extend_from_slice(&f32::MAX.to_le_bytes()); // range[0]
        buf.extend_from_slice(&f32::MAX.to_le_bytes()); // range[1]
        buf.extend_from_slice(&0f32.to_le_bytes()); // cone
        for _ in 0..5 {
            buf.extend_from_slice(&(-1f32).to_le_bytes()); // _tail
        }

        // Each light is exactly 148 bytes.
        assert_eq!(buf.len(), 4 + 148);

        let lgt = read_lgt(&mut Cursor::new(buf)).unwrap();
        assert_eq!(lgt.light_count, 1);
        assert_eq!(lgt.lights.len(), 1);

        let l = &lgt.lights[0];
        assert_eq!(l.light_type, 1);
        assert_eq!(l.flag, 1);
        assert_eq!(l.position(), [376.605, 977.196, -2223.708]);
        assert_eq!(l.color, [0.129, 0.134, 0.136]);
        assert!(l.range[0].is_infinite() || l.range[0] == f32::MAX);
    }
}
