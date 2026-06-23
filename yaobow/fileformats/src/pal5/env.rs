//! PAL5 `envinfo.env` per-map atmosphere decoder.
//!
//! Each map ships `Map/<map>/envinfo.env`: a GameBox container
//! (`magic = 0x0001e240`, version 20) like `.mp`/`.nod`, but its body is
//! stored **uncompressed** (the file's packed and unpacked sizes are
//! always equal). The 12-byte GameBox header is followed by a fixed
//! lighting/atmosphere header and — on some maps — a variable trailing
//! region of per-map light/material records.
//!
//! ## Body layout (after the 12-byte GameBox header, little-endian)
//!
//! All offsets are relative to the body (file offset `12 +`).
//!
//! | offset | type      | field |
//! |--------|-----------|-------|
//! | 0x00   | f32 × 3   | ambient light color (RGB, 0..1) |
//! | 0x0c   | f32 × 3   | sun/diffuse light color (RGB, 0..1) |
//! | 0x18   | f32       | sun azimuth (degrees, around the +Y up axis) |
//! | 0x1c   | f32       | sun elevation (degrees above the horizon) |
//! | 0x20   | f32 × 4   | reserved (zero on every sampled map) |
//! | 0x30   | u8 × 4    | fog color RGBA (alpha is always `0xff`) |
//! | 0x34   | u32       | `0xcccccccc` padding (MSVC uninit-memory fill) |
//! | 0x38   | f32       | fog parameter A (0..1; density?) |
//! | 0x3c   | f32       | fog parameter B (0..1; far/end ratio?) |
//! | 0x40   | u32       | build tag (a year, 2003..2026, on most maps) |
//! | 0x44…  | variable  | per-map light/material records (undecoded) |
//!
//! These offsets are verified clean-room across all 139 shipped
//! `envinfo.env` files: the `0xcccccccc` sentinel sits at body `0x34` on
//! every file, the fog alpha byte is `0xff` on every file, and ambient,
//! sun, and both fog parameters fall in `[0, 1]` everywhere. The
//! `0xcccccccc` fill is the MSVC debug uninitialized-stack pattern, which
//! confirms `0x30` (fog color) and `0x38` (fog params) are distinct
//! serialized fields separated by uninitialized padding.
//!
//! The "dim ambient + bright sun" split is unambiguous across maps — e.g.
//! `shushan` = ambient (0.39, 0.38, 0.44) + sun (0.95, 0.90, 0.74),
//! `kuangfengzhai` = ambient (0.80, 0.78, 0.75) + sun (1, 1, 1). Sun
//! azimuth/elevation are genuinely per-map on real scenes (`dianchi`
//! az=266/el=76, `jiujiaomigong` az=180/el=90 overhead, `kaifeng`
//! az=50/el=45); the `battlemap_*` set legitimately shares a fixed
//! az=0 / el=43.
//!
//! The trailing region (body `0x44+`) holds per-map light/material
//! records — recurring `RGB 0xcc` color brackets plus interleaved
//! world-space positions that read as point lights — but its exact record
//! stride/count was not pinned down confidently and it is left undecoded.
//! The base atmosphere consumed by the renderer (ambient + directional
//! sun + fog color/params) lives entirely in the fixed header.

use serde::Serialize;

/// Header magic shared by PAL5 GameBox containers (`.mp`/`.nod`/`.env`).
const GAMEBOX_MAGIC: u32 = 0x0001_e240;
/// Size of the GameBox container header preceding the body.
const GAMEBOX_HEADER: usize = 12;
/// Smallest body that still contains the full fixed header (through the
/// build tag at body `0x40`). The smallest shipped file is 123 bytes
/// (body 111), so every real file clears this; the guard only protects
/// against truncated/garbage input.
const FIXED_HEADER_LEN: usize = 0x44;

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
    /// Fog color (RGBA, 0..1). The shipped alpha byte is always `0xff`,
    /// so `fog_color[3]` is `1.0` on every map.
    pub fog_color: [f32; 4],
    /// Fog parameter A (0..1). Likely a density/strength factor; stored
    /// for later use but not yet consumed by the renderer.
    pub fog_param_a: f32,
    /// Fog parameter B (0..1). Likely a far/end ratio; stored but unused.
    pub fog_param_b: f32,
    /// Build tag stored at body `0x40`. On most maps this is a plausible
    /// year (2003..2026); kept raw because a handful of maps store some
    /// other value here.
    pub build_tag: u32,
}

impl EnvFile {
    pub fn read(data: &[u8]) -> anyhow::Result<Self> {
        if data.len() < GAMEBOX_HEADER + FIXED_HEADER_LEN {
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
        let u = |off: usize| -> u32 {
            let b = GAMEBOX_HEADER + off;
            u32::from_le_bytes([data[b], data[b + 1], data[b + 2], data[b + 3]])
        };
        let byte = |off: usize| -> u8 { data[GAMEBOX_HEADER + off] };

        let fog_color = [
            byte(0x30) as f32 / 255.0,
            byte(0x31) as f32 / 255.0,
            byte(0x32) as f32 / 255.0,
            byte(0x33) as f32 / 255.0,
        ];

        Ok(Self {
            ambient: [f(0x00), f(0x04), f(0x08)],
            sun_color: [f(0x0c), f(0x10), f(0x14)],
            sun_azimuth_deg: f(0x18),
            sun_elevation_deg: f(0x1c),
            fog_color,
            fog_param_a: f(0x38),
            fog_param_b: f(0x3c),
            build_tag: u(0x40),
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
    fn make_env(
        ambient: [f32; 3],
        sun: [f32; 3],
        az: f32,
        el: f32,
        fog_rgba: [u8; 4],
        fog_a: f32,
        fog_b: f32,
        tag: u32,
    ) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&GAMEBOX_MAGIC.to_le_bytes()); // magic
        v.extend_from_slice(&20u32.to_le_bytes()); // version
        v.extend_from_slice(&0u32.to_le_bytes()); // uncompressed size
        // body starts here (offset 12)
        for c in ambient.iter().chain(sun.iter()) {
            v.extend_from_slice(&c.to_le_bytes()); // 0x00..0x18
        }
        v.extend_from_slice(&az.to_le_bytes()); // 0x18
        v.extend_from_slice(&el.to_le_bytes()); // 0x1c
        v.resize(GAMEBOX_HEADER + 0x30, 0); // 0x20..0x30 reserved zeros
        v.extend_from_slice(&fog_rgba); // 0x30 fog color
        v.extend_from_slice(&0xccccccccu32.to_le_bytes()); // 0x34 sentinel
        v.extend_from_slice(&fog_a.to_le_bytes()); // 0x38
        v.extend_from_slice(&fog_b.to_le_bytes()); // 0x3c
        v.extend_from_slice(&tag.to_le_bytes()); // 0x40
        v
    }

    #[test]
    fn decodes_header_fields() {
        let raw = make_env(
            [0.39, 0.38, 0.44],
            [0.95, 0.90, 0.74],
            168.0,
            40.0,
            [0x26, 0x2c, 0x00, 0xff],
            0.01,
            0.45,
            2020,
        );
        let env = EnvFile::read(&raw).unwrap();
        assert!((env.ambient[0] - 0.39).abs() < 1e-6);
        assert!((env.sun_color[2] - 0.74).abs() < 1e-6);
        assert!((env.sun_azimuth_deg - 168.0).abs() < 1e-6);
        assert!((env.sun_elevation_deg - 40.0).abs() < 1e-6);
        assert!((env.fog_color[0] - 0x26 as f32 / 255.0).abs() < 1e-6);
        assert!((env.fog_color[3] - 1.0).abs() < 1e-6, "alpha 0xff -> 1.0");
        assert!((env.fog_param_a - 0.01).abs() < 1e-6);
        assert!((env.fog_param_b - 0.45).abs() < 1e-6);
        assert_eq!(env.build_tag, 2020);
    }

    #[test]
    fn decodes_real_map_sun_angles() {
        // dianchi-like: per-map azimuth/elevation, not the battlemap default.
        let raw = make_env(
            [0.27, 0.39, 0.32],
            [0.42, 0.97, 0.98],
            266.0,
            76.0,
            [0x10, 0x20, 0x30, 0xff],
            0.0,
            0.5,
            2020,
        );
        let env = EnvFile::read(&raw).unwrap();
        assert!((env.sun_azimuth_deg - 266.0).abs() < 1e-6);
        assert!((env.sun_elevation_deg - 76.0).abs() < 1e-6);
    }

    #[test]
    fn sun_direction_points_up_for_overhead() {
        let raw = make_env(
            [0.5; 3],
            [1.0; 3],
            0.0,
            90.0,
            [0, 0, 0, 0xff],
            0.0,
            0.0,
            0,
        );
        let env = EnvFile::read(&raw).unwrap();
        let d = env.sun_direction();
        assert!(d[1] > 0.999, "overhead sun should point +Y, got {d:?}");
    }

    #[test]
    fn rejects_bad_magic() {
        let mut raw = make_env([0.5; 3], [1.0; 3], 0.0, 45.0, [0, 0, 0, 0xff], 0.0, 0.0, 0);
        raw[0] = 0xff;
        assert!(EnvFile::read(&raw).is_err());
    }

    #[test]
    fn rejects_truncated() {
        assert!(EnvFile::read(&[0u8; 8]).is_err());
        // Long enough for the old (0x20) header but short of the fog/tag
        // fields: must still be rejected rather than read out of bounds.
        assert!(EnvFile::read(&[0u8; GAMEBOX_HEADER + 0x20]).is_err());
    }
}
