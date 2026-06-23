//! PAL5 `envinfo.env` per-map atmosphere decoder.
//!
//! Each map ships `Map/<map>/envinfo.env`: a GameBox container
//! (`magic = 0x0001e240`, version 20) like `.mp`/`.nod`, but its body is
//! stored **uncompressed** (the file's packed and unpacked sizes are
//! always equal). The 12-byte GameBox header is followed by a fixed
//! lighting/atmosphere header and — on some maps — a variable list of
//! per-map point lights.
//!
//! ## Header layout (after the 12-byte GameBox header, little-endian)
//!
//! | offset | type      | field |
//! |--------|-----------|-------|
//! | 0x00   | f32 × 3   | ambient light color (RGB, 0..1) |
//! | 0x0c   | f32 × 3   | sun/diffuse light color (RGB, 0..1) |
//! | 0x18   | f32       | sun azimuth (degrees, around the +Y up axis) |
//! | 0x1c   | f32       | sun elevation (degrees above the horizon) |
//!
//! These offsets are relative to the body (file offset `12 +`). The
//! remaining header bytes (fog color, view distance, year tag, per-map
//! point-light records) are not needed for base atmosphere and are left
//! undecoded.
//!
//! Reverse-engineered clean-room from the shipped `.env` files: the
//! "dim ambient + bright sun" split is unambiguous across maps — e.g.
//! `shushan` = ambient (0.39, 0.38, 0.44) + sun (0.95, 0.90, 0.74),
//! `kuangfengzhai` = ambient (0.80, 0.78, 0.75) + sun (1, 1, 1) — and the
//! two trailing scalars fall in plausible azimuth (0..360) / elevation
//! (0..90) ranges on every map.

use serde::Serialize;

/// Header magic shared by PAL5 GameBox containers (`.mp`/`.nod`/`.env`).
const GAMEBOX_MAGIC: u32 = 0x0001_e240;
/// Size of the GameBox container header preceding the body.
const GAMEBOX_HEADER: usize = 12;

#[derive(Debug, Clone, Serialize)]
pub struct EnvFile {
    /// Ambient light color (RGB, linear 0..1).
    pub ambient: [f32; 3],
    /// Sun/diffuse light color (RGB, linear 0..1).
    pub sun_color: [f32; 3],
    /// Sun azimuth in degrees (compass angle around the +Y up axis).
    pub sun_azimuth_deg: f32,
    /// Sun elevation in degrees above the horizon.
    pub sun_elevation_deg: f32,
}

impl EnvFile {
    pub fn read(data: &[u8]) -> anyhow::Result<Self> {
        if data.len() < GAMEBOX_HEADER + 0x20 {
            anyhow::bail!("envinfo.env too small: {} bytes", data.len());
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != GAMEBOX_MAGIC {
            anyhow::bail!("envinfo.env bad magic: 0x{magic:08x}");
        }

        let f = |off: usize| -> f32 {
            let b = GAMEBOX_HEADER + off;
            f32::from_le_bytes([data[b], data[b + 1], data[b + 2], data[b + 3]])
        };

        Ok(Self {
            ambient: [f(0x00), f(0x04), f(0x08)],
            sun_color: [f(0x0c), f(0x10), f(0x14)],
            sun_azimuth_deg: f(0x18),
            sun_elevation_deg: f(0x1c),
        })
    }

    /// Unit direction **from the ground toward the sun**, in the engine's
    /// left-handed Y-up world space. Elevation lifts the vector off the
    /// XZ plane; azimuth rotates it around +Y.
    pub fn sun_direction(&self) -> [f32; 3] {
        let az = self.sun_azimuth_deg.to_radians();
        let el = self.sun_elevation_deg.to_radians();
        let cos_el = el.cos();
        [cos_el * az.cos(), el.sin(), cos_el * az.sin()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid `.env` body from header field values.
    fn make_env(ambient: [f32; 3], sun: [f32; 3], az: f32, el: f32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&GAMEBOX_MAGIC.to_le_bytes()); // magic
        v.extend_from_slice(&20u32.to_le_bytes()); // version
        v.extend_from_slice(&0u32.to_le_bytes()); // uncompressed
        for c in ambient.iter().chain(sun.iter()) {
            v.extend_from_slice(&c.to_le_bytes());
        }
        v.extend_from_slice(&az.to_le_bytes());
        v.extend_from_slice(&el.to_le_bytes());
        v.resize(GAMEBOX_HEADER + 0x40, 0); // pad past the decoded header
        v
    }

    #[test]
    fn decodes_header_fields() {
        let raw = make_env([0.39, 0.38, 0.44], [0.95, 0.90, 0.74], 168.0, 40.0);
        let env = EnvFile::read(&raw).unwrap();
        assert!((env.ambient[0] - 0.39).abs() < 1e-6);
        assert!((env.sun_color[2] - 0.74).abs() < 1e-6);
        assert!((env.sun_azimuth_deg - 168.0).abs() < 1e-6);
        assert!((env.sun_elevation_deg - 40.0).abs() < 1e-6);
    }

    #[test]
    fn sun_direction_points_up_for_overhead() {
        let raw = make_env([0.5; 3], [1.0; 3], 0.0, 90.0);
        let env = EnvFile::read(&raw).unwrap();
        let d = env.sun_direction();
        assert!(d[1] > 0.999, "overhead sun should point +Y, got {d:?}");
    }

    #[test]
    fn rejects_bad_magic() {
        let mut raw = make_env([0.5; 3], [1.0; 3], 0.0, 45.0);
        raw[0] = 0xff;
        assert!(EnvFile::read(&raw).is_err());
    }

    #[test]
    fn rejects_truncated() {
        assert!(EnvFile::read(&[0u8; 8]).is_err());
    }
}
