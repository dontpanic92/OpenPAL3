//! Inline markup parser for PAL4 dialog text.
//!
//! PAL4 dialog payloads embed CEGUI-style inline markup:
//!
//! * `<colour red=R green=G blue=B alpha=A>…</colour>` — an explicit
//!   RGBA run, components are 0..255 decimal.
//! * `<dcN>…</dcN>` — "default colour N", a palette reference into the
//!   dialog widget's `TextColours` (loaded from the CEGUI `.layout`).
//!   The imgui port doesn't currently plumb that palette through, so
//!   every `<dcN>` falls back to the dialog widget's default text
//!   colour (passed in by the caller as `default_color`). TODO: when
//!   `yaobow/shared/src/loaders/cegui/layout.rs::WindowDef::text_colours`
//!   is wired into [`crate::ui::dialog_box::DialogBox`], thread the
//!   parsed palette into [`parse`].
//!
//! Behaviour notes:
//!
//! * Tags are matched case-insensitively. `<colour>` attributes are
//!   order-independent; missing attributes default (`alpha=255`, others
//!   `0`). Out-of-range components are clamped to `0..=255`.
//! * Tag stack underflows are silently ignored — dialog text is data
//!   we don't fully control, so we never panic on malformed input.
//! * Unknown tags (e.g. `<font>`, `<image>`) pass through literally so
//!   the player still sees the content, and are logged once per parse
//!   at `log::debug!` level so unknown markup gets noticed during
//!   development.
//! * Consecutive same-colour text segments are coalesced so the
//!   renderer's per-character layout loop stays cheap.

use std::borrow::Cow;

/// A coloured run of text. The renderer walks these and lays them out
/// character-by-character with `text_colored` + `same_line(0., 0.)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub text: String,
    pub color: [f32; 4],
}

/// Parse PAL4 dialog markup into a vector of coloured segments.
///
/// `default_color` is the widget's "base" text colour: it's the colour
/// used for any run not wrapped in a `<colour>` or `<dcN>` tag, and is
/// also the placeholder colour returned for every `<dcN>` lookup until
/// the real palette is wired through.
pub fn parse(text: &str, default_color: [f32; 4]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut color_stack: Vec<[f32; 4]> = vec![default_color];
    let mut buf = String::new();

    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = find_tag_end(bytes, i) {
                let raw = &text[i + 1..end];
                match classify_tag(raw) {
                    Tag::OpenColour(rgba) => {
                        flush(&mut out, &mut buf, *color_stack.last().unwrap());
                        color_stack.push(rgba);
                        i = end + 1;
                        continue;
                    }
                    Tag::OpenDcN(_n) => {
                        // TODO: look up palette[n] from the dialog
                        // widget's TextColours once that's plumbed
                        // through DialogBox. Until then every <dcN>
                        // resolves to the widget's default colour.
                        flush(&mut out, &mut buf, *color_stack.last().unwrap());
                        color_stack.push(default_color);
                        i = end + 1;
                        continue;
                    }
                    Tag::Close => {
                        flush(&mut out, &mut buf, *color_stack.last().unwrap());
                        if color_stack.len() > 1 {
                            color_stack.pop();
                        }
                        i = end + 1;
                        continue;
                    }
                    Tag::Unknown => {
                        log::debug!("dialog_markup: passing through unknown tag <{}>", raw);
                        buf.push_str(&text[i..=end]);
                        i = end + 1;
                        continue;
                    }
                }
            }
            // Unterminated `<`: treat literally and advance one byte.
            buf.push('<');
            i += 1;
        } else {
            // Append one UTF-8 scalar so we don't split mid-codepoint.
            let ch_len = utf8_char_len(bytes[i]);
            buf.push_str(&text[i..i + ch_len]);
            i += ch_len;
        }
    }

    flush(&mut out, &mut buf, *color_stack.last().unwrap());
    out
}

/// Return `text` with every recognised markup tag removed. Used by the
/// dialog state snapshot so external consumers (the agent server's
/// `/v1/state.dialog.text`, debug overlays) get clean text without
/// having to parse markup themselves.
pub fn strip(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = find_tag_end(bytes, i) {
                let raw = &text[i + 1..end];
                if matches!(classify_tag(raw), Tag::Unknown) {
                    out.push_str(&text[i..=end]);
                }
                i = end + 1;
                continue;
            }
            out.push('<');
            i += 1;
        } else {
            let ch_len = utf8_char_len(bytes[i]);
            out.push_str(&text[i..i + ch_len]);
            i += ch_len;
        }
    }
    out
}

enum Tag {
    OpenColour([f32; 4]),
    OpenDcN(u8),
    Close,
    Unknown,
}

fn classify_tag(raw: &str) -> Tag {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix('/') {
        let name = stripped.trim().to_ascii_lowercase();
        return if name == "colour"
            || name == "color"
            || (name.starts_with("dc") && name[2..].chars().all(|c| c.is_ascii_digit()))
        {
            Tag::Close
        } else {
            Tag::Unknown
        };
    }

    // Split tag name from its attribute list.
    let (name, rest) = match trimmed.find(char::is_whitespace) {
        Some(idx) => (&trimmed[..idx], &trimmed[idx..]),
        None => (trimmed, ""),
    };
    let name_lc = name.to_ascii_lowercase();

    if name_lc == "colour" || name_lc == "color" {
        return Tag::OpenColour(parse_colour_attrs(rest));
    }

    if name_lc.starts_with("dc") {
        if let Ok(n) = name_lc[2..].parse::<u8>() {
            return Tag::OpenDcN(n);
        }
    }

    Tag::Unknown
}

fn parse_colour_attrs(attrs: &str) -> [f32; 4] {
    let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 255u32);
    for token in attrs.split_whitespace() {
        if let Some((key, value)) = token.split_once('=') {
            let value: u32 = value.trim_matches('"').parse().unwrap_or(0).min(255);
            match key.trim().to_ascii_lowercase().as_str() {
                "red" | "r" => r = value,
                "green" | "g" => g = value,
                "blue" | "b" => b = value,
                "alpha" | "a" => a = value,
                _ => {}
            }
        }
    }
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

fn find_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert_eq!(bytes[start], b'<');
    bytes[start + 1..]
        .iter()
        .position(|&b| b == b'>')
        .map(|off| start + 1 + off)
}

fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte {
        b if b < 0x80 => 1,
        b if b & 0xE0 == 0xC0 => 2,
        b if b & 0xF0 == 0xE0 => 3,
        b if b & 0xF8 == 0xF0 => 4,
        _ => 1, // Invalid lead byte: advance one and let later code recover.
    }
}

fn flush(out: &mut Vec<Segment>, buf: &mut String, color: [f32; 4]) {
    if buf.is_empty() {
        return;
    }
    let text: Cow<str> = Cow::Owned(std::mem::take(buf));
    if let Some(last) = out.last_mut() {
        if last.color == color {
            last.text.push_str(&text);
            return;
        }
    }
    out.push(Segment {
        text: text.into_owned(),
        color,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

    #[test]
    fn bug_report_payload_produces_three_runs() {
        let raw = "云天河：\n<colour red=255 green=187 blue=0 alpha=255>先回房拿弓，</colour><dc0>再去石沉溪洞猎山猪～</dc0>";
        let segs = parse(raw, WHITE);
        assert_eq!(segs.len(), 3, "got {:?}", segs);
        assert_eq!(segs[0].text, "云天河：\n");
        assert_eq!(segs[0].color, WHITE);
        assert_eq!(segs[1].text, "先回房拿弓，");
        assert_eq!(segs[1].color, [1.0, 187.0 / 255.0, 0.0, 1.0]);
        assert_eq!(segs[2].text, "再去石沉溪洞猎山猪～");
        assert_eq!(segs[2].color, WHITE);
    }

    #[test]
    fn nested_colour_runs_stack_correctly() {
        let raw = "a<colour red=255 green=0 blue=0 alpha=255>b<colour red=0 green=255 blue=0 alpha=255>c</colour>d</colour>e";
        let segs = parse(raw, WHITE);
        assert_eq!(segs.len(), 5);
        assert_eq!(segs[0].text, "a");
        assert_eq!(segs[0].color, WHITE);
        assert_eq!(segs[1].text, "b");
        assert_eq!(segs[1].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(segs[2].text, "c");
        assert_eq!(segs[2].color, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(segs[3].text, "d");
        assert_eq!(segs[3].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(segs[4].text, "e");
        assert_eq!(segs[4].color, WHITE);
    }

    #[test]
    fn unclosed_colour_survives_to_end_of_string() {
        let raw = "a<colour red=255 green=0 blue=0 alpha=255>tail";
        let segs = parse(raw, WHITE);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].text, "tail");
        assert_eq!(segs[1].color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn unknown_tag_passes_through_literally() {
        let raw = "before<font name=foo>after";
        let segs = parse(raw, WHITE);
        // Tag survives in the visible buffer so the player sees something.
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "before<font name=foo>after");
        assert_eq!(segs[0].color, WHITE);
    }

    #[test]
    fn dc_closing_tag_pops_even_without_matching_open() {
        let raw = "alone</dc0>tail";
        let segs = parse(raw, WHITE);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "alonetail");
    }

    #[test]
    fn strip_removes_known_tags_only() {
        let raw = "云天河：\n<colour red=255 green=187 blue=0 alpha=255>先回房拿弓，</colour><dc0>再去石沉溪洞猎山猪～</dc0>";
        assert_eq!(strip(raw), "云天河：\n先回房拿弓，再去石沉溪洞猎山猪～");
    }

    #[test]
    fn strip_preserves_unknown_tags() {
        assert_eq!(strip("a<font name=foo>b"), "a<font name=foo>b");
    }

    #[test]
    fn empty_input_yields_no_segments() {
        assert!(parse("", WHITE).is_empty());
        assert_eq!(strip(""), "");
    }

    #[test]
    fn newline_inside_segment_is_preserved() {
        let raw = "<colour red=255 green=0 blue=0 alpha=255>line1\nline2</colour>";
        let segs = parse(raw, WHITE);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "line1\nline2");
    }

    #[test]
    fn case_insensitive_tag_matching() {
        let raw = "<COLOR Red=255 Green=0 Blue=0 Alpha=255>x</COLOR>";
        let segs = parse(raw, WHITE);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "x");
        assert_eq!(segs[0].color, [1.0, 0.0, 0.0, 1.0]);
    }
}
