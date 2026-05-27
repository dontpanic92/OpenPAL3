//! CEGUI-style `.skin` parser (PAL4 `gamedata/ui2/ui/skin/<WidgetType>/*.skin`).
//!
//! PAL4's `.skin` files are flat associations between a widget type's
//! named components (e.g. `ButtonNormal`, `ButtonPushed`) and an
//! imageset + image:
//!
//! ```xml
//! <Skin Name="OLButton">
//!     <Self>
//!         <Component Name="ButtonNormal"    Imageset="OiramLook" Image="ButtonNormal" />
//!         <Component Name="ButtonPushed"    Imageset="OiramLook" Image="ButtonPushed" />
//!         <Component Name="ButtonHighlight" Imageset="OiramLook" Image="ButtonHighlight" />
//!         <Component Name="ButtonDisabled"  Imageset="OiramLook" Image="ButtonDisabled" />
//!     </Self>
//!     <Child Name="ChildName" Skin="ChildSkinName" />
//! </Skin>
//! ```
//!
//! Skins act as the per-widget-type fallback when a layout omits an
//! `ol*Image` property — the previewer can fall back to e.g.
//! `ButtonNormal` from the matching `OlButton.skin` for an
//! unconfigured `OiramLook/Button`. The current PAL4 layouts always
//! set their own `ol*Image` properties so this fallback is rarely
//! exercised, but the parser is here for completeness.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use common::store_ext::StoreExt2;
use mini_fs::MiniFs;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SkinComponent {
    pub name: String,
    pub imageset: String,
    pub image: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkinChild {
    pub name: String,
    pub skin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skin {
    pub name: String,
    /// Components keyed by lower-cased name for case-insensitive lookup.
    pub components: HashMap<String, SkinComponent>,
    pub children: Vec<SkinChild>,
}

impl Skin {
    pub fn component(&self, name: &str) -> Option<&SkinComponent> {
        self.components.get(&name.to_lowercase())
    }
}

pub fn parse_skin_str(content: &str) -> Result<Skin> {
    let doc = roxmltree::Document::parse(content)?;
    let root = doc.root_element();
    if root.tag_name().name() != "Skin" {
        return Err(anyhow!(
            "expected root element <Skin>, got <{}>",
            root.tag_name().name()
        ));
    }
    let name = root.attribute("Name").unwrap_or("").to_string();
    let mut components: HashMap<String, SkinComponent> = HashMap::new();
    let mut children: Vec<SkinChild> = Vec::new();
    for top in root.children().filter(|n| n.is_element()) {
        match top.tag_name().name() {
            "Self" => {
                for c in top.children().filter(|n| n.is_element()) {
                    if c.tag_name().name() != "Component" {
                        continue;
                    }
                    let cname = c.attribute("Name").unwrap_or("").to_string();
                    let imageset = c.attribute("Imageset").unwrap_or("").to_string();
                    let image = c.attribute("Image").unwrap_or("").to_string();
                    components.insert(
                        cname.to_lowercase(),
                        SkinComponent {
                            name: cname,
                            imageset,
                            image,
                        },
                    );
                }
            }
            "Child" => children.push(SkinChild {
                name: top.attribute("Name").unwrap_or("").to_string(),
                skin: top.attribute("Skin").unwrap_or("").to_string(),
            }),
            _ => {}
        }
    }
    Ok(Skin {
        name,
        components,
        children,
    })
}

pub fn load_skin(vfs: &MiniFs, vfs_path: &str) -> Result<Skin> {
    let bytes = vfs.read_to_end(vfs_path)?;
    parse_skin_str(&String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_olbutton_skin() {
        let s = r#"<Skin Name="OLButton">
<Self>
<Component Name="ButtonDisabled" Imageset="OiramLook" Image="ButtonDisabled" />
<Component Name="ButtonHighlight" Imageset="OiramLook" Image="ButtonHighlight" />
<Component Name="ButtonNormal" Imageset="OiramLook" Image="ButtonNormal" />
<Component Name="ButtonPushed" Imageset="OiramLook" Image="ButtonPushed" />
</Self>
</Skin>"#;
        let p = parse_skin_str(s).expect("parse");
        assert_eq!(p.name, "OLButton");
        assert_eq!(p.components.len(), 4);
        let normal = p.component("buttonnormal").expect("ButtonNormal");
        assert_eq!(normal.imageset, "OiramLook");
        assert_eq!(normal.image, "ButtonNormal");
        // Case-insensitive lookup.
        assert!(p.component("BUTTONnormal").is_some());
        assert!(p.component("missing").is_none());
    }

    #[test]
    fn parses_skin_with_children() {
        let s = r#"<Skin Name="OLFrameWindow">
<Self><Component Name="ClientBrush" Imageset="OiramLook" Image="ClientBrush" /></Self>
<Child Name="FrameWindowCloseButton" Skin="OLFrameWindow-FrameWindowCloseButton" />
<Child Name="FrameWindowTitleBar" Skin="OLFrameWindow-FrameWindowTitleBar" />
</Skin>"#;
        let p = parse_skin_str(s).expect("parse");
        assert_eq!(p.children.len(), 2);
        assert_eq!(p.children[0].name, "FrameWindowCloseButton");
        assert_eq!(p.children[0].skin, "OLFrameWindow-FrameWindowCloseButton");
    }
}
