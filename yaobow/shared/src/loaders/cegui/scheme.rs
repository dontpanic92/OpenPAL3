//! CEGUI `.scheme` XML parser (PAL4 `gamedata/ui2/ui/schemes/*.scheme`).
//!
//! PAL4 ships a single `OiramLook.scheme` that lists the imagesets and
//! windowset (looknfeel/skin family) the layouts reference:
//!
//! ```xml
//! <GUIScheme Name="OiramLook">
//!     <Imageset Name="OiramLook" Filename="gamedata/ui/imagesets/OiramLook.imageset" />
//!     <WindowSet Filename="OiramLook" />
//! </GUIScheme>
//! ```
//!
//! The previewer never *needs* the scheme today (layouts reference
//! imagesets directly via `set:<Name>` and the editor resolves them by
//! scanning the imagesets directory), but exposing the parser keeps
//! M2's surface complete and gives the inspector view something
//! human-readable for the file.

use anyhow::{Result, anyhow};
use common::store_ext::StoreExt2;
use mini_fs::MiniFs;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SchemeImageset {
    pub name: String,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Scheme {
    pub name: String,
    pub imagesets: Vec<SchemeImageset>,
    pub windowsets: Vec<String>,
}

pub fn parse_scheme_str(content: &str) -> Result<Scheme> {
    let doc = roxmltree::Document::parse(content)?;
    let root = doc.root_element();
    if root.tag_name().name() != "GUIScheme" {
        return Err(anyhow!(
            "expected root element <GUIScheme>, got <{}>",
            root.tag_name().name()
        ));
    }
    let name = root.attribute("Name").unwrap_or("").to_string();
    let mut imagesets = Vec::new();
    let mut windowsets = Vec::new();
    for child in root.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "Imageset" => imagesets.push(SchemeImageset {
                name: child.attribute("Name").unwrap_or("").to_string(),
                filename: child.attribute("Filename").unwrap_or("").to_string(),
            }),
            "WindowSet" => {
                windowsets.push(child.attribute("Filename").unwrap_or("").to_string());
            }
            _ => {}
        }
    }
    Ok(Scheme {
        name,
        imagesets,
        windowsets,
    })
}

pub fn load_scheme(vfs: &MiniFs, vfs_path: &str) -> Result<Scheme> {
    let bytes = vfs.read_to_end(vfs_path)?;
    parse_scheme_str(&String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_pal4_scheme() {
        let s = r#"<?xml version="1.0" ?>
<GUIScheme Name="OiramLook">
<Imageset Name="OiramLook" Filename="gamedata/ui/imagesets/OiramLook.imageset" />
<WindowSet Filename="OiramLook" />
</GUIScheme>"#;
        let p = parse_scheme_str(s).expect("parse");
        assert_eq!(p.name, "OiramLook");
        assert_eq!(p.imagesets.len(), 1);
        assert_eq!(p.imagesets[0].name, "OiramLook");
        assert_eq!(p.windowsets, vec!["OiramLook"]);
    }
}
