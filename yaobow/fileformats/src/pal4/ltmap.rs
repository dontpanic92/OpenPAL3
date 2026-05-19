//! PAL4 `<block>_ltMap.cfg` — per-scene lightmap modulation config.
//!
//! 16 bytes, four little-endian IEEE-754 floats: `(r, g, b, intensity)`.
//! The first three are an RGB tint multiplier applied to the baked
//! `*LightingMap.dds`; the fourth is a scalar intensity / ambient term
//! used (most prominently) to dim night variants of the same scene
//! (`Q01` vs `Q01Y`, `N02` vs `N02Y`, …). See `generated/ltmap.md` for
//! the full reverse-engineering write-up.
//!
//! The shipped renderer consumes this as
//! `final = (lightMap * 1.5 + 0.15) * diffuse * tint.rgb * intensity`
//! — i.e. it fits straight into the existing `lightmap_texture.frag`
//! per-material UBO once `MaterialParams.tint` is set to
//! `[r, g, b, intensity]`.

use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};

/// Per-scene lightmap modulation config (16 bytes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LtMapCfg {
    /// RGB tint applied to baked lightmap samples. Each channel sits in
    /// `[0, 1]` in every shipped file; at least one channel is exactly
    /// `1.0` (max-brightness tint convention).
    pub tint: [f32; 3],

    /// Scalar intensity / ambient term, in `[0, 1]`. Drops below `0.5`
    /// for the night variants of paired day/night scenes.
    pub intensity: f32,
}

impl LtMapCfg {
    /// Identity modulation: white tint, full intensity. Used as the
    /// fallback when a scene does not ship a `_ltMap.cfg`.
    pub const IDENTITY: Self = Self {
        tint: [1.0, 1.0, 1.0],
        intensity: 1.0,
    };

    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let r = reader.read_f32::<LittleEndian>()?;
        let g = reader.read_f32::<LittleEndian>()?;
        let b = reader.read_f32::<LittleEndian>()?;
        let w = reader.read_f32::<LittleEndian>()?;
        Ok(Self {
            tint: [r, g, b],
            intensity: w,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn encode(r: f32, g: f32, b: f32, w: f32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16);
        buf.extend_from_slice(&r.to_le_bytes());
        buf.extend_from_slice(&g.to_le_bytes());
        buf.extend_from_slice(&b.to_le_bytes());
        buf.extend_from_slice(&w.to_le_bytes());
        buf
    }

    /// `Q01` (day, warm): rgb ≈ (0.98, 1.00, 0.85), intensity ≈ 0.76
    /// per `generated/ltmap.md`.
    #[test]
    fn parses_q01_day_sample() {
        let buf = encode(0.98, 1.00, 0.85, 0.76);
        let cfg = LtMapCfg::read(&mut Cursor::new(buf)).unwrap();
        assert!((cfg.tint[0] - 0.98).abs() < 1e-5);
        assert!((cfg.tint[1] - 1.00).abs() < 1e-5);
        assert!((cfg.tint[2] - 0.85).abs() < 1e-5);
        assert!((cfg.intensity - 0.76).abs() < 1e-5);
    }

    /// `Q01Y` (night, cool blue): rgb ≈ (0.52, 0.76, 1.00),
    /// intensity ≈ 0.40 — the controlled-experiment day↔night pair.
    #[test]
    fn parses_q01y_night_sample() {
        let buf = encode(0.52, 0.76, 1.00, 0.40);
        let cfg = LtMapCfg::read(&mut Cursor::new(buf)).unwrap();
        assert!(cfg.tint[2] > cfg.tint[0]); // blue-shifted vs day
        assert!(cfg.intensity < 0.5);
    }

    #[test]
    fn identity_is_no_op() {
        assert_eq!(LtMapCfg::IDENTITY.tint, [1.0, 1.0, 1.0]);
        assert_eq!(LtMapCfg::IDENTITY.intensity, 1.0);
    }

    #[test]
    fn rejects_truncated_input() {
        let buf = vec![0u8; 12]; // 12 bytes instead of 16
        assert!(LtMapCfg::read(&mut Cursor::new(buf)).is_err());
    }
}
