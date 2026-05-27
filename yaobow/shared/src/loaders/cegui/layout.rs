//! CEGUI `<GUILayout>` XML parser (PAL4 `gamedata/ui/layouts/*.xml`,
//! delivered inside `gamedata/ui.cpk` and mounted at `/gamedata/ui/`).
//!
//! Each layout is a recursive tree of `<Window>` nodes whose visual
//! appearance comes from `<Property Name="..." Value="..."/>` children.
//! For the editor previewer we only care about a small set of
//! properties:
//!
//! - `UnifiedAreaRect` — position + size in the imageset's native
//!   canvas (e.g. 800x600). Format:
//!   `{{xs,xo},{ys,yo},{xe_s,xe_o},{ye_s,ye_o}}` — each pair is
//!   `scale * parent_extent + offset`. PAL4's layouts use scale=0
//!   everywhere so we keep the parsed scales around for future use
//!   but rely on offsets for v1 rendering.
//! - `Image` / `olNormalImage` — `set:<SetName> image:<ImageName>`.
//!   Identifies the imageset region to blit at the window's rect.
//! - `Text`, `Font`, `TextColours` — text properties (consumed in M3).
//! - `Visible`, `AlwaysOnTop`, `Disabled` — boolean state flags.
//!
//! All other properties are preserved verbatim in `Window::properties`
//! so the inspector view (and M2/M3) can read them without re-parsing.

use anyhow::{anyhow, Result};
use common::store_ext::StoreExt2;
use mini_fs::MiniFs;
use serde::Serialize;

/// CEGUI-style "unified" scalar: `scale * parent_extent + offset`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct UnifiedDim {
    pub scale: f32,
    pub offset: f32,
}

impl UnifiedDim {
    pub fn resolve(self, parent_extent: f32) -> f32 {
        self.scale * parent_extent + self.offset
    }
}

/// CEGUI `UnifiedAreaRect`: top-left + bottom-right in unified
/// coordinates. We store the four corner dims rather than (pos, size)
/// because that is the wire format.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct UnifiedAreaRect {
    pub left: UnifiedDim,
    pub top: UnifiedDim,
    pub right: UnifiedDim,
    pub bottom: UnifiedDim,
}

impl UnifiedAreaRect {
    /// Resolve to a `(x, y, w, h)` pixel rect against a parent canvas
    /// of the given size.
    pub fn resolve(&self, parent_w: f32, parent_h: f32) -> (f32, f32, f32, f32) {
        let x0 = self.left.resolve(parent_w);
        let y0 = self.top.resolve(parent_h);
        let x1 = self.right.resolve(parent_w);
        let y1 = self.bottom.resolve(parent_h);
        (x0, y0, x1 - x0, y1 - y0)
    }
}

/// `set:<SetName> image:<ImageName>` reference parsed from an `Image`
/// (or `olNormalImage` / similar) property value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImageRef {
    pub set: String,
    pub image: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Property {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Window {
    /// Pre-order traversal index (root = 0).
    pub id: u32,
    /// Parent's traversal index, or `None` for the root.
    pub parent: Option<u32>,
    /// CEGUI window type, e.g. `DefaultWindow`, `OiramLook/StaticImage`.
    pub window_type: String,
    pub name: String,
    pub area: UnifiedAreaRect,
    pub image: Option<ImageRef>,
    pub overlay_normal_image: Option<ImageRef>,
    pub overlay_highlight_image: Option<ImageRef>,
    pub overlay_pushed_image: Option<ImageRef>,
    pub overlay_disabled_image: Option<ImageRef>,
    pub text: Option<String>,
    pub font: Option<String>,
    pub text_colours: Option<String>,
    pub visible: bool,
    pub disabled: bool,
    pub always_on_top: bool,
    /// All raw `<Property>` rows in document order, preserved for the
    /// inspector view and for M2/M3.
    pub properties: Vec<Property>,
    /// Indices into `LayoutFile::windows` of this window's direct
    /// children, in document order.
    pub children: Vec<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutFile {
    /// Flat pre-order list of every `<Window>` in the document. Index
    /// 0 is the root. Children are referenced by index via
    /// `Window::children`.
    pub windows: Vec<Window>,
}

impl LayoutFile {
    pub fn root(&self) -> Option<&Window> {
        self.windows.first()
    }
}

/// Parse a layout from XML bytes.
pub fn parse_layout_bytes(bytes: &[u8]) -> Result<LayoutFile> {
    let content = String::from_utf8_lossy(bytes);
    parse_layout_str(&content)
}

/// Parse a layout from an XML string.
pub fn parse_layout_str(content: &str) -> Result<LayoutFile> {
    let doc = roxmltree::Document::parse(content)?;
    let root = doc.root_element();
    if root.tag_name().name() != "GUILayout" {
        return Err(anyhow!(
            "expected root element <GUILayout>, got <{}>",
            root.tag_name().name()
        ));
    }

    let mut windows: Vec<Window> = Vec::new();
    // The root <GUILayout> may host multiple sibling <Window>s; PAL4
    // ships a single root window per layout, but we accept multiple
    // and chain them off a synthetic root if encountered.
    let top_windows: Vec<_> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Window")
        .collect();
    if top_windows.is_empty() {
        return Err(anyhow!("<GUILayout> has no <Window> children"));
    }
    if top_windows.len() == 1 {
        read_window(top_windows[0], None, &mut windows);
    } else {
        // Synthetic root that mirrors the first child's area so the
        // layout still has a single Window[0] entry.
        let first = top_windows[0];
        let first_area = read_area(first);
        windows.push(Window {
            id: 0,
            parent: None,
            window_type: "DefaultWindow".to_string(),
            name: "<synthetic-root>".to_string(),
            area: first_area,
            image: None,
            overlay_normal_image: None,
            overlay_highlight_image: None,
            overlay_pushed_image: None,
            overlay_disabled_image: None,
            text: None,
            font: None,
            text_colours: None,
            visible: true,
            disabled: false,
            always_on_top: false,
            properties: Vec::new(),
            children: Vec::new(),
        });
        for w in top_windows {
            let child_id = read_window(w, Some(0), &mut windows);
            windows[0].children.push(child_id);
        }
    }

    Ok(LayoutFile { windows })
}

fn read_area(node: roxmltree::Node) -> UnifiedAreaRect {
    node.children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Property")
        .find(|p| p.attribute("Name") == Some("UnifiedAreaRect"))
        .and_then(|p| p.attribute("Value"))
        .and_then(|v| parse_unified_area_rect(v).ok())
        .unwrap_or_default()
}

fn read_window(
    node: roxmltree::Node,
    parent: Option<u32>,
    out: &mut Vec<Window>,
) -> u32 {
    let id = out.len() as u32;
    let window_type = node
        .attribute("Type")
        .unwrap_or("DefaultWindow")
        .to_string();
    let name = node.attribute("Name").unwrap_or("").to_string();

    let mut window = Window {
        id,
        parent,
        window_type,
        name,
        area: UnifiedAreaRect::default(),
        image: None,
        overlay_normal_image: None,
        overlay_highlight_image: None,
        overlay_pushed_image: None,
        overlay_disabled_image: None,
        text: None,
        font: None,
        text_colours: None,
        visible: true,
        disabled: false,
        always_on_top: false,
        properties: Vec::new(),
        children: Vec::new(),
    };
    out.push(window.clone()); // placeholder so id slot is reserved

    // Read properties + child windows in document order.
    let mut child_ids: Vec<u32> = Vec::new();
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "Property" => {
                let pname = match child.attribute("Name") {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let pvalue = child.attribute("Value").unwrap_or("").to_string();

                match pname.as_str() {
                    "UnifiedAreaRect" => {
                        if let Ok(rect) = parse_unified_area_rect(&pvalue) {
                            window.area = rect;
                        }
                    }
                    "Image" => {
                        window.image = parse_image_ref(&pvalue);
                    }
                    "olNormalImage" => {
                        window.overlay_normal_image = parse_image_ref(&pvalue);
                    }
                    "olHighlightImage" => {
                        window.overlay_highlight_image = parse_image_ref(&pvalue);
                    }
                    "olPushedImage" => {
                        window.overlay_pushed_image = parse_image_ref(&pvalue);
                    }
                    "olDisabledImage" => {
                        window.overlay_disabled_image = parse_image_ref(&pvalue);
                    }
                    "Text" => window.text = Some(pvalue.clone()),
                    "Font" => window.font = Some(pvalue.clone()),
                    "TextColours" => window.text_colours = Some(pvalue.clone()),
                    "Visible" => window.visible = parse_bool(&pvalue, true),
                    "Disabled" => window.disabled = parse_bool(&pvalue, false),
                    "AlwaysOnTop" => window.always_on_top = parse_bool(&pvalue, false),
                    _ => {}
                }
                window.properties.push(Property {
                    name: pname,
                    value: pvalue,
                });
            }
            "Window" => {
                // Recurse — the child's id is the next slot. We must
                // commit the current window's state back to `out`
                // before recursing because the child also pushes into
                // `out` while we still hold a local copy.
                out[id as usize] = window.clone();
                let child_id = read_window(child, Some(id), out);
                // Reload our local copy in case the recursion did not
                // touch our slot (it won't, because read_window only
                // mutates its own slot + descendants).
                window = out[id as usize].clone();
                child_ids.push(child_id);
            }
            _ => {}
        }
    }

    window.children = child_ids;
    out[id as usize] = window;
    id
}

/// Parse `{{xs,xo},{ys,yo},{xe_s,xe_o},{ye_s,ye_o}}` into a rect.
pub fn parse_unified_area_rect(s: &str) -> Result<UnifiedAreaRect> {
    let pairs = parse_dim_pairs(s)?;
    if pairs.len() != 4 {
        return Err(anyhow!(
            "expected 4 unified dim pairs in UnifiedAreaRect, got {}",
            pairs.len()
        ));
    }
    Ok(UnifiedAreaRect {
        left: pairs[0],
        top: pairs[1],
        right: pairs[2],
        bottom: pairs[3],
    })
}

fn parse_dim_pairs(s: &str) -> Result<Vec<UnifiedDim>> {
    // Extract floats; pair them up as (scale, offset). We deliberately
    // ignore braces and commas — the format is too rigid to merit a
    // proper grammar. CEGUI emits ASCII; chardet won't get involved.
    let mut nums: Vec<f32> = Vec::new();
    let mut start: Option<usize> = None;
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        let is_num =
            b.is_ascii_digit() || b == b'-' || b == b'+' || b == b'.' || b == b'e' || b == b'E';
        if is_num && start.is_none() {
            start = Some(i);
        } else if !is_num && start.is_some() {
            let s_idx = start.take().unwrap();
            if let Ok(n) = s[s_idx..i].parse::<f32>() {
                nums.push(n);
            }
        }
    }
    if let Some(s_idx) = start {
        if let Ok(n) = s[s_idx..].parse::<f32>() {
            nums.push(n);
        }
    }
    if nums.len() % 2 != 0 {
        return Err(anyhow!(
            "odd number of floats ({}) in unified dim list: {:?}",
            nums.len(),
            s
        ));
    }
    Ok(nums
        .chunks_exact(2)
        .map(|c| UnifiedDim {
            scale: c[0],
            offset: c[1],
        })
        .collect())
}

/// Parse `set:<SetName> image:<ImageName>` into an `ImageRef`.
/// Tolerates extra whitespace and missing fields; returns `None` if
/// either segment is missing.
pub fn parse_image_ref(s: &str) -> Option<ImageRef> {
    let mut set = None;
    let mut image = None;
    // Tokens are space-separated; each token is `key:value`.
    for token in s.split_whitespace() {
        let mut parts = token.splitn(2, ':');
        let key = parts.next()?;
        let value = match parts.next() {
            Some(v) => v.to_string(),
            None => continue,
        };
        match key {
            "set" => set = Some(value),
            "image" => image = Some(value),
            _ => {}
        }
    }
    Some(ImageRef {
        set: set?,
        image: image?,
    })
}

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => true,
        "false" | "0" | "no" => false,
        _ => default,
    }
}

/// Open a `<GUILayout>` file from the vfs.
pub fn load_layout(vfs: &MiniFs, vfs_path: &str) -> Result<LayoutFile> {
    let bytes = vfs.read_to_end(vfs_path)?;
    parse_layout_bytes(&bytes)
}

/// Cheap classifier: returns true when `bytes` is XML whose first
/// non-whitespace element is `<GUILayout`. Tolerates a leading `<?xml ?>`
/// prolog and an optional UTF-8 BOM.
pub fn looks_like_gui_layout(bytes: &[u8]) -> bool {
    // Skip UTF-8 BOM.
    let mut s = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };
    // Skip whitespace.
    while let Some((&b, rest)) = s.split_first() {
        if b.is_ascii_whitespace() {
            s = rest;
        } else {
            break;
        }
    }
    // Optional <?xml ... ?> prolog.
    if s.starts_with(b"<?xml") {
        if let Some(end) = s.windows(2).position(|w| w == b"?>") {
            s = &s[end + 2..];
        }
    }
    while let Some((&b, rest)) = s.split_first() {
        if b.is_ascii_whitespace() {
            s = rest;
        } else {
            break;
        }
    }
    // Optional XML comments before the root element.
    while s.starts_with(b"<!--") {
        if let Some(end) = s.windows(3).position(|w| w == b"-->") {
            s = &s[end + 3..];
            while let Some((&b, rest)) = s.split_first() {
                if b.is_ascii_whitespace() {
                    s = rest;
                } else {
                    break;
                }
            }
        } else {
            return false;
        }
    }
    s.starts_with(b"<GUILayout")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<GUILayout>
<Window Type="DefaultWindow" Name="Root">
<Property Name="UnifiedAreaRect" Value="{{0.000000,0.000000},{0.000000,0.000000},{0.000000,800.000000},{0.000000,600.000000}}" />
<Window Type="OiramLook/StaticImage" Name="Bg">
<Property Name="Image" Value="set:duihuakuang0 image:jingdutiao" />
<Property Name="UnifiedAreaRect" Value="{{0.000000,10.000000},{0.000000,20.000000},{0.000000,110.000000},{0.000000,120.000000}}" />
<Property Name="AlwaysOnTop" Value="True" />
</Window>
<Window Type="OiramLook/StaticImage" Name="Bg2">
<Property Name="Image" Value="set:OiramLook image:ClientBrush" />
<Property Name="UnifiedAreaRect" Value="{{0,30},{0,40},{0,130},{0,140}}" />
</Window>
</Window>
</GUILayout>"#;

    #[test]
    fn parses_basic_layout() {
        let layout = parse_layout_str(SAMPLE).expect("parse");
        assert_eq!(layout.windows.len(), 3);
        let root = layout.root().expect("root");
        assert_eq!(root.window_type, "DefaultWindow");
        assert_eq!(root.name, "Root");
        assert_eq!(root.children.len(), 2);
        let (x, y, w, h) = root.area.resolve(800.0, 600.0);
        assert_eq!((x, y, w, h), (0.0, 0.0, 800.0, 600.0));

        let bg = &layout.windows[root.children[0] as usize];
        assert_eq!(bg.parent, Some(0));
        assert_eq!(bg.window_type, "OiramLook/StaticImage");
        assert_eq!(bg.name, "Bg");
        assert!(bg.always_on_top);
        let img = bg.image.as_ref().expect("image ref");
        assert_eq!(img.set, "duihuakuang0");
        assert_eq!(img.image, "jingdutiao");
        let (x, y, w, h) = bg.area.resolve(800.0, 600.0);
        assert_eq!((x, y, w, h), (10.0, 20.0, 100.0, 100.0));

        let bg2 = &layout.windows[root.children[1] as usize];
        assert!(!bg2.always_on_top);
        assert_eq!(bg2.image.as_ref().unwrap().set, "OiramLook");
    }

    #[test]
    fn looks_like_gui_layout_detects_real_files() {
        assert!(looks_like_gui_layout(SAMPLE.as_bytes()));
        assert!(looks_like_gui_layout(
            b"<?xml version=\"1.0\"?>\n<GUILayout>...</GUILayout>"
        ));
        assert!(looks_like_gui_layout(b"<GUILayout/>"));
        assert!(!looks_like_gui_layout(b"<Imageset/>"));
        assert!(!looks_like_gui_layout(b"not xml at all"));
        // UTF-8 BOM tolerated.
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(SAMPLE.as_bytes());
        assert!(looks_like_gui_layout(&with_bom));
    }

    #[test]
    fn parse_image_ref_robust() {
        assert_eq!(
            parse_image_ref("set:Foo image:Bar"),
            Some(ImageRef {
                set: "Foo".into(),
                image: "Bar".into()
            })
        );
        assert_eq!(parse_image_ref("set:Foo"), None);
        assert_eq!(parse_image_ref(""), None);
    }

    #[test]
    fn parse_unified_area_rect_robust() {
        let rect = parse_unified_area_rect(
            "{{0.5,1.0},{0,2},{1,3},{1,4}}",
        )
        .expect("parse");
        assert_eq!(rect.left, UnifiedDim { scale: 0.5, offset: 1.0 });
        assert_eq!(rect.top, UnifiedDim { scale: 0.0, offset: 2.0 });
        assert_eq!(rect.right, UnifiedDim { scale: 1.0, offset: 3.0 });
        assert_eq!(rect.bottom, UnifiedDim { scale: 1.0, offset: 4.0 });

        // Wrong number of dims should error.
        assert!(parse_unified_area_rect("{0,0}").is_err());
    }
}
