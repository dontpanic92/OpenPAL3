//! CEGUI `.imageset` XML parser.
//!
//! An imageset is a flat list of `<Image>` rectangles inside a single
//! atlas PNG. PAL4 ships imagesets inside `gamedata/ui.cpk` (mounted
//! at `/gamedata/ui/`), with portraits under `/gamedata/ui/portrait/`
//! and the main UI atlases under `/gamedata/ui/imagesets/`. The format
//! is: a root `<Imageset>` with `Name`, `Imagefile`, `NativeHorzRes`,
//! `NativeVertRes`, `AutoScaled` attributes, plus one or more
//! `<Image>` children carrying `Name`/`XPos`/`YPos`/`Width`/`Height`
//! (and optional `XOffset`/`YOffset`).
//!
//! Example (from `gamedata/ui2/ui/imagesets/OiramLook.imageset`):
//!
//! ```xml
//! <Imageset Name="OiramLook"
//!           Imagefile="gamedata\ui\imagesets\OiramLook.png"
//!           NativeHorzRes="800" NativeVertRes="600"
//!           AutoScaled="true">
//!     <Image Name="ClientBrush" XPos="2" YPos="2" Width="64" Height="64"/>
//!     ...
//! </Imageset>
//! ```
//!
//! Image names are matched case-insensitively (the runtime PAL4 portrait
//! loader already does this, and layout XML files mix cases for the
//! same image — e.g. `image:ClientBrush` vs `image:clientbrush`).

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use common::store_ext::StoreExt2;
use mini_fs::MiniFs;

/// One `<Image>` rectangle inside an imageset atlas.
#[derive(Debug, Clone)]
pub struct ImagesetImage {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Parsed `.imageset` file. `images` is keyed by the lower-cased image
/// name to match the relaxed casing CEGUI uses at lookup time.
#[derive(Debug, Clone)]
pub struct Imageset {
    pub name: String,
    /// Absolute vfs path to the atlas PNG (the original `Imagefile`
    /// attribute, with `\` normalised to `/` and a leading `/`
    /// prepended if missing).
    pub image_path: String,
    /// Native UI canvas size declared by the imageset. Layout files
    /// position widgets in this coordinate space; the editor previewer
    /// uses it as the fitted-aspect reference (e.g. 800x600 for PAL4).
    pub native_horz_res: u32,
    pub native_vert_res: u32,
    pub auto_scaled: bool,
    pub images: HashMap<String, ImagesetImage>,
}

impl Imageset {
    pub fn get(&self, name: &str) -> Option<&ImagesetImage> {
        self.images.get(&name.to_lowercase())
    }
}

/// Parse an imageset from in-memory XML bytes.
pub fn parse_imageset_bytes(bytes: &[u8]) -> Result<Imageset> {
    let content = String::from_utf8_lossy(bytes);
    parse_imageset_str(&content)
}

/// Parse an imageset from an XML string.
pub fn parse_imageset_str(content: &str) -> Result<Imageset> {
    let doc = roxmltree::Document::parse(content)?;
    let root = doc.root_element();
    if root.tag_name().name() != "Imageset" {
        return Err(anyhow!(
            "expected root element <Imageset>, got <{}>",
            root.tag_name().name()
        ));
    }

    let name = root
        .attribute("Name")
        .ok_or_else(|| anyhow!("Missing Name attribute on <Imageset>"))?
        .to_string();
    let image_file = root
        .attribute("Imagefile")
        .ok_or_else(|| anyhow!("Missing Imagefile attribute on <Imageset>"))?;
    let image_path = normalise_vfs_path(image_file);
    let native_horz_res = root
        .attribute("NativeHorzRes")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(800);
    let native_vert_res = root
        .attribute("NativeVertRes")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(600);
    let auto_scaled = root
        .attribute("AutoScaled")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut images = HashMap::new();
    for image in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Image")
    {
        let read = || -> Result<ImagesetImage> {
            let raw_name = image
                .attribute("Name")
                .ok_or_else(|| anyhow!("Missing Name in <Image>"))?;
            let x = image
                .attribute("XPos")
                .ok_or_else(|| anyhow!("Missing XPos in <Image>"))?
                .parse::<u32>()?;
            let y = image
                .attribute("YPos")
                .ok_or_else(|| anyhow!("Missing YPos in <Image>"))?
                .parse::<u32>()?;
            let width = image
                .attribute("Width")
                .ok_or_else(|| anyhow!("Missing Width in <Image>"))?
                .parse::<u32>()?;
            let height = image
                .attribute("Height")
                .ok_or_else(|| anyhow!("Missing Height in <Image>"))?
                .parse::<u32>()?;
            Ok(ImagesetImage {
                name: raw_name.to_string(),
                x,
                y,
                width,
                height,
            })
        };

        match read() {
            Ok(rect) => {
                images.insert(rect.name.to_lowercase(), rect);
            }
            Err(e) => log::warn!("imageset {}: skipping <Image>: {:#}", name, e),
        }
    }

    Ok(Imageset {
        name,
        image_path,
        native_horz_res,
        native_vert_res,
        auto_scaled,
        images,
    })
}

/// Open an imageset by vfs path and return both the parsed metadata and
/// the raw PNG bytes referenced by its `Imagefile` attribute.
///
/// Tries the literal `Imagefile` path first; if that fails, falls back
/// to the basename of the `Imagefile` joined to the `.imageset` file's
/// own directory.
pub fn load_imageset_with_atlas(vfs: &MiniFs, vfs_path: &str) -> Result<(Imageset, Vec<u8>)> {
    let xml_bytes = vfs.read_to_end(vfs_path)?;
    let imageset = parse_imageset_bytes(&xml_bytes)?;
    let png_bytes = read_atlas_png(vfs, vfs_path, &imageset)?;
    Ok((imageset, png_bytes))
}

/// Read the atlas PNG declared by an `Imageset`, with a sibling-of-
/// `.imageset` fallback. Public so `UiLayoutHandle` can reuse the
/// fallback even when it already holds a parsed `Imageset`.
///
/// The fallback exists because the uncompressed dev/portable extract
/// of PAL4 contains `.imageset` files whose `Imagefile` attribute
/// points at the original archived path (e.g.
/// `gamedata\ui\imagesets\foo.png`) rather than the colocated PNG
/// next to the `.imageset` itself. Inside `ui.cpk` the declared path
/// matches the mounted path, so the literal read succeeds and the
/// fallback is a no-op.
pub fn read_atlas_png(
    vfs: &MiniFs,
    imageset_vfs_path: &str,
    imageset: &Imageset,
) -> Result<Vec<u8>> {
    // 1) Literal path declared in the .imageset.
    if let Ok(b) = vfs.read_to_end(&imageset.image_path) {
        return Ok(b);
    }
    // 2) Same directory as the .imageset, using the declared file's
    //    basename. PAL4 needs this for every layout-time imageset.
    let basename = std::path::Path::new(&imageset.image_path)
        .file_name()
        .and_then(|n| n.to_str());
    if let Some(name) = basename {
        let dir = std::path::Path::new(imageset_vfs_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("/"));
        let fallback = dir.join(name);
        if let Some(p) = fallback.to_str() {
            if let Ok(b) = vfs.read_to_end(p) {
                return Ok(b);
            }
        }
    }
    Err(anyhow!(
        "atlas PNG not found for imageset {} (declared at '{}', sibling fallback also failed)",
        imageset.name,
        imageset.image_path
    ))
}

/// Open an imageset by vfs path; does NOT read the atlas PNG.
pub fn load_imageset(vfs: &MiniFs, vfs_path: &str) -> Result<Imageset> {
    let xml_bytes = vfs.read_to_end(vfs_path)?;
    parse_imageset_bytes(&xml_bytes)
}

/// Normalise a CEGUI `Imagefile`/path attribute into a vfs-style
/// absolute path: convert backslashes, prepend a leading slash if
/// missing.
pub fn normalise_vfs_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    if path.starts_with('/') {
        path
    } else {
        format!("/{}", path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" ?>
<Imageset Name="OiramLook" Imagefile="gamedata\ui\imagesets\OiramLook.png"
          NativeHorzRes="800" NativeVertRes="600" AutoScaled="true">
    <Image Name="ClientBrush" XPos="2" YPos="2" Width="64" Height="64" />
    <Image Name="ButtonNormal" XPos="68" YPos="20" Width="40" Height="16" />
</Imageset>"#;

    #[test]
    fn parses_basic_imageset() {
        let imageset = parse_imageset_str(SAMPLE).expect("parse");
        assert_eq!(imageset.name, "OiramLook");
        assert_eq!(imageset.image_path, "/gamedata/ui/imagesets/OiramLook.png");
        assert_eq!(imageset.native_horz_res, 800);
        assert_eq!(imageset.native_vert_res, 600);
        assert!(imageset.auto_scaled);
        assert_eq!(imageset.images.len(), 2);

        let client = imageset.get("clientbrush").expect("clientbrush");
        assert_eq!(
            (client.x, client.y, client.width, client.height),
            (2, 2, 64, 64)
        );

        // Case-insensitive lookup.
        assert!(imageset.get("ClientBrush").is_some());
        assert!(imageset.get("CLIENTBRUSH").is_some());
        assert!(imageset.get("missing").is_none());
    }

    #[test]
    fn rejects_wrong_root() {
        let xml = r#"<?xml version="1.0"?><NotAnImageset/>"#;
        assert!(parse_imageset_str(xml).is_err());
    }

    #[test]
    fn normalises_paths() {
        assert_eq!(
            normalise_vfs_path("gamedata\\ui\\x.png"),
            "/gamedata/ui/x.png"
        );
        assert_eq!(normalise_vfs_path("/already/abs.png"), "/already/abs.png");
        assert_eq!(normalise_vfs_path("a/b"), "/a/b");
    }
}
