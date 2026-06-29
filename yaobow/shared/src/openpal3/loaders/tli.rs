//! Parser for PAL3's `__PAL(Softstar_Ltd)__TEXTURE_LIBRARY_INFO__BY_COOL_J`
//! text manifest (`ui\\UILib\\UI_opt.tli`).
//!
//! The file is a GBK-encoded, CRLF-delimited list of records like:
//!
//! ```text
//! __PAL(Softstar_Ltd)__TEXTURE_LIBRARY_INFO__BY_COOL_J
//!
//! t_name      ui\Menu\beijing.tga
//! t_lib       11.tga
//! t_libw      1024
//! t_libh      1024
//! t_orix      0
//! t_oriy      0
//! t_w         800
//! t_h         600
//! t_m         0
//! #End
//! ```
//!
//! Each entry maps a logical sprite path (`t_name`) to a sub-rect inside an
//! atlas page (`t_lib`, size `t_libw`×`t_libh`, top-left `t_orix`/`t_oriy`,
//! size `t_w`×`t_h`).
//!
//! There is no layout file in PAL3 game data — this manifest is the *only*
//! UI metadata that ships. Screen positions are hard-coded in `pal3.dll`
//! and must be re-authored on the consumer side.

use std::collections::HashMap;

use encoding::{DecoderTrap, Encoding, all::GBK};
use serde::Serialize;

/// One sprite record from `UI_opt.tli`.
#[derive(Debug, Clone, Serialize)]
pub struct TliEntry {
    /// Original logical path as written in the TLI, e.g.
    /// `ui\\Menu\\beijing.tga`. Kept verbatim for diagnostics.
    pub name: String,
    /// Atlas file name (e.g. `11.tga`). Resolved against `ui\\UILib\\`.
    pub lib: String,
    pub lib_w: u32,
    pub lib_h: u32,
    pub orix: u32,
    pub oriy: u32,
    pub w: u32,
    pub h: u32,
    /// `t_m` flag. Meaning unknown; preserved for round-tripping.
    pub m: i32,
}

impl TliEntry {
    /// UV sub-rect inside the atlas page, in `[0,1]`.
    pub fn uv(&self) -> (f32, f32, f32, f32) {
        let lw = self.lib_w.max(1) as f32;
        let lh = self.lib_h.max(1) as f32;
        let u0 = self.orix as f32 / lw;
        let v0 = self.oriy as f32 / lh;
        let u1 = (self.orix + self.w) as f32 / lw;
        let v1 = (self.oriy + self.h) as f32 / lh;
        (u0, v0, u1, v1)
    }
}

/// Parsed `UI_opt.tli`. Lookups are case-insensitive and slash-agnostic.
#[derive(Debug, Default, Clone, Serialize)]
pub struct TliDict {
    entries: HashMap<String, TliEntry>,
}

impl TliDict {
    /// Parse a UI_opt.tli payload. `bytes` is the raw on-disk content
    /// (GBK-encoded); the parser tolerates either LF or CRLF terminators
    /// and any leading/trailing whitespace.
    pub fn parse(bytes: &[u8]) -> Self {
        let text = GBK
            .decode(bytes, DecoderTrap::Replace)
            .unwrap_or_else(|_| String::new());
        Self::parse_str(&text)
    }

    pub fn parse_str(text: &str) -> Self {
        let mut entries: HashMap<String, TliEntry> = HashMap::new();
        let mut cur: Option<PartialEntry> = None;

        for line in text.lines() {
            let line = line.trim_end_matches(['\r', '\n']).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("#End") {
                if let Some(entry) = cur.take().and_then(PartialEntry::finish) {
                    entries.insert(canonical_key(&entry.name), entry);
                }
                continue;
            }
            if line.starts_with("__PAL") {
                continue;
            }
            // key<whitespace>value. Split at the first whitespace; some
            // records use a single tab, some use two. `splitn(2,
            // char::is_whitespace)` collapses all of them into "rest".
            let mut parts = line.splitn(2, char::is_whitespace);
            let key = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            if key.is_empty() {
                continue;
            }
            let entry = cur.get_or_insert_with(PartialEntry::default);
            entry.set(key, value);
        }

        // Flush a trailing record that lacked an `#End` marker.
        if let Some(entry) = cur.take().and_then(PartialEntry::finish) {
            entries.insert(canonical_key(&entry.name), entry);
        }

        Self { entries }
    }

    /// Look up by logical path. Accepts either `\\` or `/` separators
    /// and is case-insensitive.
    pub fn get(&self, name: &str) -> Option<&TliEntry> {
        self.entries.get(&canonical_key(name))
    }

    /// Insert an entry under its canonical key. Used by alternate
    /// manifest parsers (e.g. PAL3A `UIArtist.plug`) that build the same
    /// dict from a different on-disk layout.
    pub fn insert(&mut self, entry: TliEntry) {
        let key = canonical_key(&entry.name);
        self.entries.insert(key, entry);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &TliEntry)> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Distinct atlas file names referenced by any entry, with the
    /// canonical key form used by `get` (lowercase, forward slashes).
    /// Each entry is `(lib_name_as_written, lib_w, lib_h)` for the
    /// first record that referenced it.
    pub fn distinct_libs(&self) -> Vec<(String, u32, u32)> {
        let mut seen: HashMap<String, (String, u32, u32)> = HashMap::new();
        for entry in self.entries.values() {
            let key = entry.lib.to_lowercase();
            seen.entry(key)
                .or_insert_with(|| (entry.lib.clone(), entry.lib_w, entry.lib_h));
        }
        seen.into_values().collect()
    }
}

fn canonical_key(name: &str) -> String {
    name.replace('\\', "/").to_lowercase()
}

#[derive(Default)]
struct PartialEntry {
    name: Option<String>,
    lib: Option<String>,
    lib_w: Option<u32>,
    lib_h: Option<u32>,
    orix: Option<u32>,
    oriy: Option<u32>,
    w: Option<u32>,
    h: Option<u32>,
    m: Option<i32>,
}

impl PartialEntry {
    fn set(&mut self, key: &str, value: &str) {
        match key {
            "t_name" => self.name = Some(value.to_string()),
            "t_lib" => self.lib = Some(value.to_string()),
            "t_libw" => self.lib_w = value.parse().ok(),
            "t_libh" => self.lib_h = value.parse().ok(),
            "t_orix" => self.orix = value.parse().ok(),
            "t_oriy" => self.oriy = value.parse().ok(),
            "t_w" => self.w = value.parse().ok(),
            "t_h" => self.h = value.parse().ok(),
            "t_m" => self.m = value.parse().ok(),
            _ => {}
        }
    }

    fn finish(self) -> Option<TliEntry> {
        Some(TliEntry {
            name: self.name?,
            lib: self.lib?,
            lib_w: self.lib_w.unwrap_or(0),
            lib_h: self.lib_h.unwrap_or(0),
            orix: self.orix.unwrap_or(0),
            oriy: self.oriy.unwrap_or(0),
            w: self.w.unwrap_or(0),
            h: self.h.unwrap_or(0),
            m: self.m.unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "__PAL(Softstar_Ltd)__TEXTURE_LIBRARY_INFO__BY_COOL_J\r\n\r\nt_name\t\tui\\Menu\\beijing.tga\r\nt_lib\t\t11.tga\r\nt_libw\t\t1024\r\nt_libh\t\t1024\r\nt_orix\t\t0\r\nt_oriy\t\t0\r\nt_w\t\t800\r\nt_h\t\t600\r\nt_m\t\t0\r\n#End\r\n\r\nt_name\t\tui\\Menu\\btn_qianchenyimeng0.tga\r\nt_lib\t\t11.tga\r\nt_libw\t\t1024\r\nt_libh\t\t1024\r\nt_orix\t\t215\r\nt_oriy\t\t0\r\nt_w\t\t128\r\nt_h\t\t256\r\nt_m\t\t0\r\n#End\r\n";

    #[test]
    fn parses_two_entries() {
        let d = TliDict::parse_str(SAMPLE);
        assert_eq!(d.len(), 2);

        let bg = d.get("ui/menu/beijing.tga").expect("bg");
        assert_eq!(bg.lib, "11.tga");
        assert_eq!(bg.lib_w, 1024);
        assert_eq!(bg.w, 800);
        assert_eq!(bg.h, 600);

        // Backslash + uppercase lookups must hit.
        assert!(d.get("UI\\Menu\\beijing.TGA").is_some());

        let btn = d.get("ui/menu/btn_qianchenyimeng0.tga").expect("btn");
        let (u0, v0, u1, v1) = btn.uv();
        assert!((u0 - 215.0 / 1024.0).abs() < 1e-6);
        assert!((v0 - 0.0).abs() < 1e-6);
        assert!((u1 - (215.0 + 128.0) / 1024.0).abs() < 1e-6);
        assert!((v1 - 256.0 / 1024.0).abs() < 1e-6);
    }

    #[test]
    fn distinct_libs_dedup() {
        let d = TliDict::parse_str(SAMPLE);
        let libs = d.distinct_libs();
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].0, "11.tga");
    }
}
