//! RenderWare UV Animation Dictionary (`.uva`).
//!
//! PAL4 ships a sibling `<block>_water.uva` next to every water `*.dff`.
//! The file is a standard-shaped RenderWare chunk container:
//!
//! ```text
//! CHUNK [UVANIMDICT (0x2B)]
//!     CHUNK [STRUCT (0x01)] body = u32 num_anims
//!     num_anims × CHUNK [ANIM_ANIMATION (0x1B)] body (152 bytes each in the
//!                                                    PAL4 samples)
//! ```
//!
//! Each animation body opens with the well-known `RtAnimAnimation` header
//! (5 × u32: `version`, `type_id`, `num_frames`, `flags`, `f32 duration`)
//! followed by a fixed `name` field used by the material to look up its
//! animation (the same string appears in the material's `PLUGIN_USERDATA`
//! `name` field inside the DFF). The keyframe block that follows
//! (`raw_keyframes`) is kept as opaque bytes by this parser — the byte
//! layout doesn't match the stock RpUVAnimLinear/Param SDK layout 1:1 and
//! is decoded later by a best-effort heuristic in the consumer code. This
//! crate's job is to recover the structural envelope reliably so callers
//! can match an animation by `name` and inspect the rest defensively.

use std::io::Read;

use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::{check_ty, ChunkHeader, ChunkType};

/// Fixed-width name field carried by each `RpUVAnim` entry. Verified
/// against every PAL4 `.uva` sample (`Q01`, `Q01Y`, `BJ`, `ZJM` waters).
pub const UV_ANIM_NAME_LEN: usize = 32;

/// Byte pattern that occupies the `f32 duration` slot of every observed
/// PAL4 `.uva` sample (Q01, Q01Y, BJ, ZJM). Spelled `"VU\x05B"` in the
/// on-disk layout (`0x42055556` little-endian); read as a float, this is
/// `≈ 33.333…` (i.e. `100/3`) seconds — the animation's full duration.
///
/// Originally treated as an opaque magic by this parser, the value is
/// retained as a soft sanity check on the duration slot. Parsing relaxes
/// to a warning + best-effort extraction when the value diverges so
/// future PAL4 assets with non-default durations don't trip the parse.
pub const UV_ANIM_VUB_MAGIC: u32 = 0x4205_5556;

#[derive(Debug, Serialize, Clone)]
pub struct UvAnim {
    /// `RtAnimAnimation::version` — `0x100` in every observed sample.
    pub version: u32,
    /// `RtAnimAnimation::type_id` — `0x1C1` in every observed sample.
    pub type_id: u32,
    /// Number of keyframes encoded in `raw_keyframes`. Two in every
    /// observed sample.
    pub num_frames: u32,
    pub flags: u32,
    /// Animation duration in seconds, read from the standard RW
    /// `RtAnimAnimation::duration` slot. In every observed PAL4 sample
    /// this value is `100/3 ≈ 33.333…` (the byte pattern that spells
    /// `"VU\x05B"`); see [`UV_ANIM_VUB_MAGIC`] for the rationale.
    pub duration: f32,
    /// Animation name. This is the lookup key that the consuming material
    /// stamps into its `PLUGIN_USERDATA name` entry inside the DFF.
    pub name: String,
    /// Raw bytes of the keyframe block that follows the animation header
    /// and the 32-byte name. Length is fixed at 152 − 20 (header) − 4
    /// (magic) − 4 (reserved) − 32 (name) = 92 bytes in every observed
    /// PAL4 sample, but the parser keeps it size-agnostic so larger files
    /// (longer animations) still round-trip cleanly.
    #[serde(skip_serializing)]
    pub raw_keyframes: Vec<u8>,
}

impl UvAnim {
    fn read(cursor: &mut dyn Read, body_len: usize) -> anyhow::Result<Self> {
        // PAL4's `RpUVAnim` animation body layout (observed across every
        // bundled `.uva` sample — Q01, Q01Y, BJ, ZJM water). This is the
        // standard RW `RtAnimAnimation` header with one quirk: the
        // `f32 duration` slot happens to spell `"VU\x05B"` as bytes
        // (`0x42055556` ≈ 33.333…s) in every observed sample, which led
        // earlier revisions to misread it as a magic separator. Modern
        // parsing reads it as a float and exposes it via `duration`; we
        // still log a warning if it diverges from the well-known value
        // so future PAL4 assets get surfaced.
        //
        //   u32 version          // = 0x100
        //   u32 type_id          // = 0x1C1
        //   u32 num_frames       // = 2
        //   u32 flags            // = 0
        //   f32 duration         // = 33.333…  (bytes spell "VU\x05B")
        //   u32 reserved         // = 0
        //   char name[32]
        //   u8  raw_keyframes[remaining]
        const FIXED_PREFIX: usize = 4 * 4 + 4 + 4 + UV_ANIM_NAME_LEN;
        if body_len < FIXED_PREFIX {
            anyhow::bail!(
                "UvAnim body too short: {} bytes (need at least {})",
                body_len,
                FIXED_PREFIX
            );
        }
        let version = cursor.read_u32_le()?;
        let type_id = cursor.read_u32_le()?;
        let num_frames = cursor.read_u32_le()?;
        let flags = cursor.read_u32_le()?;

        let duration_bits = cursor.read_u32_le()?;
        let duration = f32::from_bits(duration_bits);
        if duration_bits != UV_ANIM_VUB_MAGIC {
            log::debug!(
                "UvAnim duration slot 0x{:08X} (={}s) differs from the observed PAL4 default 0x{:08X} (={}s); accepting as-is",
                duration_bits,
                duration,
                UV_ANIM_VUB_MAGIC,
                f32::from_bits(UV_ANIM_VUB_MAGIC),
            );
        }
        // Reserved u32 immediately after the duration. Always zero in
        // observed samples; we accept any value.
        let _reserved = cursor.read_u32_le()?;

        let name = cursor.read_gbk_string(UV_ANIM_NAME_LEN)?;

        let remaining = body_len - FIXED_PREFIX;
        let raw_keyframes = cursor.read_u8_vec(remaining)?;

        Ok(Self {
            version,
            type_id,
            num_frames,
            flags,
            duration,
            name,
            raw_keyframes,
        })
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct UvAnimDict {
    pub animations: Vec<UvAnim>,
}

impl UvAnimDict {
    /// Parse a `.uva` file from its raw bytes.
    pub fn read_from_bytes(data: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(data);
        Self::read(&mut cursor)
    }

    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let header = ChunkHeader::read(cursor)?;
        check_ty!(header.ty, ChunkType::UVANIMDICT);

        let struct_header = ChunkHeader::read(cursor)?;
        check_ty!(struct_header.ty, ChunkType::STRUCT);

        // STRUCT body is a single u32 `num_anims`. Some writers also pad
        // the struct out to a larger size — read the declared length and
        // ignore any bytes beyond the first u32.
        if (struct_header.length as usize) < 4 {
            anyhow::bail!(
                "UVANIMDICT STRUCT too short: {} bytes",
                struct_header.length
            );
        }
        let num_anims = cursor.read_u32_le()?;
        let pad = (struct_header.length as usize) - 4;
        if pad > 0 {
            cursor.skip(pad)?;
        }

        let mut animations = Vec::with_capacity(num_anims as usize);
        for _ in 0..num_anims {
            let anim_header = ChunkHeader::read(cursor)?;
            check_ty!(anim_header.ty, ChunkType::ANIM_ANIMATION);
            animations.push(UvAnim::read(cursor, anim_header.length as usize)?);
        }

        Ok(Self { animations })
    }

    /// Look up an animation by its on-disk name. The lookup is
    /// case-sensitive — this matches how the consuming material in the DFF
    /// spells its `PLUGIN_USERDATA name` value.
    pub fn find(&self, name: &str) -> Option<&UvAnim> {
        self.animations.iter().find(|a| a.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `BJ_water.uva` (192 bytes, 1 animation, name
    /// `"StdMat[ 1711 ]-[ 130 ]"`). Embedded as a hex string so the test
    /// stays hermetic — round-trips the smallest known PAL4 sample end to
    /// end.
    fn bj_water_bytes() -> Vec<u8> {
        // Reproduces F:\PAL4\gamedata\ui2\ui\uiWorld\bj\BJ_water.uva
        // (192 bytes). Byte-for-byte equal to the on-disk file.
        let mut b: Vec<u8> = Vec::new();
        // --- UVANIMDICT (0x2B) header: len=0xB4 (180 = file_size − 12).
        b.extend(&0x2Bu32.to_le_bytes());
        b.extend(&0xB4u32.to_le_bytes());
        b.extend(&0x1C020065u32.to_le_bytes());
        // --- STRUCT (0x01) header, body=4 bytes (num_anims).
        b.extend(&0x01u32.to_le_bytes());
        b.extend(&4u32.to_le_bytes());
        b.extend(&0x1C020065u32.to_le_bytes());
        b.extend(&1u32.to_le_bytes()); // num_anims
        // --- ANIM_ANIMATION (0x1B) header, body=152 bytes.
        b.extend(&0x1Bu32.to_le_bytes());
        b.extend(&152u32.to_le_bytes());
        b.extend(&0x1C020065u32.to_le_bytes());
        // ANIM body: 4×u32 header (version, type_id, num_frames, flags).
        b.extend(&0x100u32.to_le_bytes());
        b.extend(&0x1C1u32.to_le_bytes());
        b.extend(&2u32.to_le_bytes());
        b.extend(&0u32.to_le_bytes());
        // VUB magic + reserved.
        b.extend(&UV_ANIM_VUB_MAGIC.to_le_bytes());
        b.extend(&0u32.to_le_bytes());
        // 32-byte name, null-padded.
        let name = b"StdMat[ 1711 ]-[ 130 ]";
        b.extend(name);
        b.extend(&vec![0u8; UV_ANIM_NAME_LEN - name.len()]);
        // 96 bytes of keyframe blob, byte-for-byte from the sample.
        b.extend(&[0u8; 36]); // body 56..91 — all zeros (32 + the 4 bytes that used to be "duration")
        // body 92..119 (28 bytes)
        b.extend(&0u32.to_le_bytes());
        b.extend(&(-0.0f32).to_le_bytes());
        b.extend(&1.0f32.to_le_bytes());
        b.extend(&1.0f32.to_le_bytes());
        b.extend(&0u32.to_le_bytes());
        b.extend(&0u32.to_le_bytes());
        b.extend(&0xFF90AE22u32.to_le_bytes());
        // body 120..151 (32 bytes) — opens with VUB magic
        b.extend(&UV_ANIM_VUB_MAGIC.to_le_bytes());
        b.extend(&(-0.0f32).to_le_bytes());
        b.extend(&1.0f32.to_le_bytes());
        b.extend(&1.0f32.to_le_bytes());
        b.extend(&0u32.to_le_bytes());
        b.extend(&(-1.0f32).to_le_bytes());
        b.extend(&1.0f32.to_le_bytes());
        b.extend(&0u32.to_le_bytes());
        debug_assert_eq!(b.len(), 192);
        b
    }

    // Legacy helper kept in case future tests want to drop in raw hex.
    #[allow(dead_code)]
    fn hex_to_bytes(s: &str) -> Vec<u8> {
        let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(cleaned.len() % 2 == 0, "odd hex length: {}", cleaned.len());
        (0..cleaned.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn parses_bj_water_dictionary() {
        let bytes = bj_water_bytes();
        assert_eq!(bytes.len(), 192, "bj_water_bytes must reproduce the sample size");
        let dict = UvAnimDict::read_from_bytes(&bytes).expect("parse BJ_water.uva");
        assert_eq!(dict.animations.len(), 1);
        let a = &dict.animations[0];
        assert_eq!(a.version, 0x100);
        assert_eq!(a.type_id, 0x1C1);
        assert_eq!(a.num_frames, 2);
        assert_eq!(a.flags, 0);
        // Duration in observed PAL4 samples = 0x42055556 as f32 = 100/3.
        assert!((a.duration - 33.333_332).abs() < 1e-3, "duration={}", a.duration);
        assert_eq!(a.name, "StdMat[ 1711 ]-[ 130 ]");
        // 152 anim body − 16 header − 4 duration − 4 reserved − 32 name = 96.
        assert_eq!(a.raw_keyframes.len(), 96);
    }

    #[test]
    fn find_by_name_matches() {
        let dict = UvAnimDict::read_from_bytes(&bj_water_bytes()).unwrap();
        assert!(dict.find("StdMat[ 1711 ]-[ 130 ]").is_some());
        assert!(dict.find("Material #1234").is_none());
    }

    #[test]
    fn rejects_wrong_top_chunk_type() {
        // STRUCT (0x01) where UVANIMDICT (0x2B) is required.
        let mut bytes: Vec<u8> = vec![];
        bytes.extend(&0x01u32.to_le_bytes());
        bytes.extend(&0u32.to_le_bytes());
        bytes.extend(&0u16.to_le_bytes());
        bytes.extend(&0u16.to_le_bytes());
        let err = UvAnimDict::read_from_bytes(&bytes).unwrap_err();
        assert!(
            format!("{err:?}").contains("Incorrect chunk"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn accepts_arbitrary_duration_value() {
        // Earlier revisions rejected anything other than the PAL4-default
        // 0x42055556 in the duration slot. We now read it as f32 and
        // surface it as `UvAnim::duration`. Build a minimal valid file
        // whose duration is a different float and confirm it round-trips.
        let body_len: u32 = 16 + 4 + 4 + UV_ANIM_NAME_LEN as u32 + 4;
        let mut anim_body: Vec<u8> = vec![];
        anim_body.extend(&0x100u32.to_le_bytes());
        anim_body.extend(&0x1C1u32.to_le_bytes());
        anim_body.extend(&1u32.to_le_bytes()); // num_frames
        anim_body.extend(&0u32.to_le_bytes()); // flags
        anim_body.extend(&5.0f32.to_le_bytes()); // duration = 5.0 seconds
        anim_body.extend(&0u32.to_le_bytes()); // reserved
        anim_body.extend(&[0u8; UV_ANIM_NAME_LEN]); // empty name
        anim_body.extend(&[0u8; 4]); // padding

        assert_eq!(anim_body.len() as u32, body_len);

        let mut buf: Vec<u8> = vec![];
        // UVANIMDICT header
        buf.extend(&0x2Bu32.to_le_bytes());
        buf.extend(&(12u32 + 4 + 12 + body_len).to_le_bytes());
        buf.extend(&0u16.to_le_bytes());
        buf.extend(&0u16.to_le_bytes());
        // STRUCT body containing num_anims = 1
        buf.extend(&0x01u32.to_le_bytes());
        buf.extend(&4u32.to_le_bytes());
        buf.extend(&0u16.to_le_bytes());
        buf.extend(&0u16.to_le_bytes());
        buf.extend(&1u32.to_le_bytes());
        // ANIM_ANIMATION header
        buf.extend(&0x1Bu32.to_le_bytes());
        buf.extend(&body_len.to_le_bytes());
        buf.extend(&0u16.to_le_bytes());
        buf.extend(&0u16.to_le_bytes());
        buf.extend(&anim_body);

        let dict = UvAnimDict::read_from_bytes(&buf).expect("parse with non-default duration");
        assert_eq!(dict.animations.len(), 1);
        assert_eq!(dict.animations[0].duration, 5.0);
    }
}
