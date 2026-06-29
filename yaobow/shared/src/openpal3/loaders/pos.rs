//! Parser for PAL3A's binary/text UI atlas manifest `ui\\UIArtist.plug`.
//!
//! PAL3A replaces PAL3's `ui\\UILib\\UI_opt.tli` with two files in `ui/`:
//! `UIArtist.plug` (the atlas manifest: sprite name → atlas sub-rect) and
//! `UIPos.pos` (a binary hash→screen-position layout, not needed to carve
//! sprites). Reverse-engineered clean-room from the shipped data: the plug
//! is a GBK, CR/LF-delimited list of fixed 12-line records:
//!
//! ```text
//! ui\gamemainui\communal\timemoney.tga   ; name (logical path)
//! 1813721796                             ; name hash (matches UIPos.pos)
//! 1                                      ; orix
//! 1                                      ; oriy
//! 203                                    ; w
//! 41                                     ; h
//! 1024                                   ; lib_w
//! 1024                                   ; lib_h
//! 1.tga                                  ; lib (atlas page)
//! 1024                                   ; (page width, ignored)
//! 1024                                   ; (page height, ignored)
//! 0                                      ; m flag
//! ```
//!
//! The whole-file leading line is a stray `0`; records then repeat. This
//! parser maps each record into a `TliEntry` so the existing `Pal3UiAtlas`
//! can carve sprites by name uniformly across PAL3 and PAL3A.

use encoding::{DecoderTrap, Encoding, all::GBK};

use super::tli::{TliDict, TliEntry};

/// Parse a `UIArtist.plug` payload into a `TliDict`. `bytes` is the raw
/// on-disk content (GBK-encoded); tolerates LF or CRLF terminators.
pub fn parse(bytes: &[u8]) -> TliDict {
    let text = GBK
        .decode(bytes, DecoderTrap::Replace)
        .unwrap_or_else(|_| String::new());
    parse_str(&text)
}

fn parse_str(text: &str) -> TliDict {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();

    let mut dict = TliDict::default();
    // A name line is any line ending in ".tga"; the 11 numeric/name fields
    // follow it. Scanning for the name anchor is robust to the stray
    // leading "0" and any header noise.
    let mut i = 0;
    while i + 11 < lines.len() {
        let name = lines[i];
        if !name.to_lowercase().ends_with(".tga") {
            i += 1;
            continue;
        }
        // i+1 hash, i+2 orix, i+3 oriy, i+4 w, i+5 h, i+6 libw, i+7 libh,
        // i+8 lib, i+9/i+10 page size (ignored), i+11 m.
        let orix = lines[i + 2].parse().unwrap_or(0);
        let oriy = lines[i + 3].parse().unwrap_or(0);
        let w = lines[i + 4].parse().unwrap_or(0);
        let h = lines[i + 5].parse().unwrap_or(0);
        let lib_w = lines[i + 6].parse().unwrap_or(0);
        let lib_h = lines[i + 7].parse().unwrap_or(0);
        let lib = lines[i + 8].to_string();
        let m = lines[i + 11].parse().unwrap_or(0);
        dict.insert(TliEntry {
            name: name.to_string(),
            lib,
            lib_w,
            lib_h,
            orix,
            oriy,
            w,
            h,
            m,
        });
        i += 12;
    }
    dict
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "0\r\nui\\gamemainui\\communal\\timemoney.tga\r\n1813721796\r\n1\r\n1\r\n203\r\n41\r\n1024\r\n1024\r\n1.tga\r\n1024\r\n1024\r\n0\r\nui\\subgame\\zshc\\hc9.tga\r\n142022303\r\n834\r\n114\r\n54\r\n55\r\n1024\r\n1024\r\n12.tga\r\n1024\r\n1024\r\n0\r\n";

    #[test]
    fn parses_two_plug_records() {
        let d = parse(SAMPLE.as_bytes());
        assert_eq!(d.len(), 2);
        let e = d.get("ui/gamemainui/communal/timemoney.tga").expect("e");
        assert_eq!(e.lib, "1.tga");
        assert_eq!(e.orix, 1);
        assert_eq!(e.oriy, 1);
        assert_eq!(e.w, 203);
        assert_eq!(e.h, 41);
        assert_eq!(e.lib_w, 1024);
        let hc = d.get("ui/subgame/zshc/hc9.tga").expect("hc");
        assert_eq!(hc.lib, "12.tga");
        assert_eq!(hc.w, 54);
    }
}
