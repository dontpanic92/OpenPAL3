//! PAL3 `<index>.lgt` — dynamic scene light table.
//!
//! Layout verified against original gbengine.dll's light loader (0x100135a0)
//! and Compute (0x10013350). Legacy public field names are retained.
//!
//! ```text
//! 0x00  u32              light_count
//! repeat light_count × 148 bytes:
//!   +0x00  f32[16]       4x4 transform (row-major); world position = row 3
//!                        (floats at +0x30,+0x34,+0x38); m33 @ +0x3C == 1.0
//!   +0x40  u32           type: 0 point, 1 spot, 2 directional
//!   +0x44  f32[4]        diffuse RGBA (also supplies enabled ambient)
//!   +0x54  f32[4]        specular RGBA
//!   +0x64  u32           diffuse-enabled flag
//!   +0x68  u32           ambient-enabled flag
//!   +0x74  f32[2]        spatial bounds, not attenuation coefficients
//!   +0x7C  u32           attenuation mode: 0 constant, 1 linear, 2 quadratic
//!   +0x80  f32           attenuation length
//! ```
//!
//! The native geometry effects evaluate all types as point lights.
//! Directional lights use a distant virtual position derived from orientation.

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

    /// Native type word, retained losslessly under its legacy name.
    pub _unknown_40: f32,

    /// RGB color / intensity (pre-scaled; the bright overhead "key" lights
    /// carry values > 1.0).
    pub color: [f32; 3],

    /// Diffuse alpha followed by specular RGBA, stored as raw float bits.
    pub _pad_50: [u32; 5],

    /// Native diffuse-enabled flag at +0x64 (legacy name).
    pub light_type: u32,

    /// Native ambient-enabled flag at +0x68 (legacy name).
    pub flag: u32,

    /// Additional native parameters, retained as raw float bits.
    pub _pad_6c: [u32; 2],

    /// Spatial bounds; most are sqrt(FLT_MAX), but many are finite.
    pub range: [f32; 2],

    /// Native attenuation mode word, retained as float bits (legacy name).
    pub cone: f32,

    /// Attenuation length at +0x80 followed by remaining native parameters,
    /// retained under the legacy field name.
    pub _tail: [f32; 5],
}

impl Light {
    pub fn diffuse_color(&self) -> [f32; 3] {
        if self.light_type != 0 {
            self.color
        } else {
            [0.; 3]
        }
    }

    pub fn ambient_color(&self) -> [f32; 3] {
        if self.flag != 0 { self.color } else { [0.; 3] }
    }

    pub fn attenuation(&self) -> [f32; 3] {
        let length = self._tail[0];
        match self.cone.to_bits() {
            1 => [0.01, 1. / length, 0.],
            2 => [0.01, 0., 1. / (length * length)],
            _ => [1., 0., 0.],
        }
    }

    /// Native type 2 transforms (0,-1,0) and scales it by -100000.
    pub fn shader_position(&self) -> [f32; 3] {
        if self._unknown_40.to_bits() == 2 {
            [
                self.transform[4] * 100000.,
                self.transform[5] * 100000.,
                self.transform[6] * 100000.,
            ]
        } else {
            self.position()
        }
    }

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

    #[test]
    fn native_light_shading_decodes_attenuation_enables_and_directional_position() {
        let mut light = Light {
            transform: [0.; 16],
            _unknown_40: 0.,
            color: [0.9, 0.6, 0.3],
            _pad_50: [0; 5],
            light_type: 1,
            flag: 1,
            _pad_6c: [0; 2],
            range: [10., 20.],
            cone: 0.,
            _tail: [119., 0., 0., 0., 0.],
        };
        light.transform[5] = 1.;
        light.transform[12..15].copy_from_slice(&[10., 20., 30.]);
        assert_eq!(light.attenuation(), [1., 0., 0.]);
        light.cone = f32::from_bits(1);
        assert_eq!(light.attenuation(), [0.01, 1. / 119., 0.]);
        light.cone = f32::from_bits(2);
        light._tail[0] = 143.;
        assert_eq!(light.attenuation(), [0.01, 0., 1. / (143. * 143.)]);
        light.flag = 0;
        assert_eq!(light.ambient_color(), [0.; 3]);
        assert_eq!(light.diffuse_color(), [0.9, 0.6, 0.3]);
        light._unknown_40 = f32::from_bits(2);
        assert_eq!(light.shader_position(), [0., 100000., 0.]);
        assert_eq!(light.position(), [10., 20., 30.]);
    }

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
