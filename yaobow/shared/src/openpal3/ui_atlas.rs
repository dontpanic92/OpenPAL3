//! `Pal3UiAtlas` — bridges PAL3's binary `UI_opt.tli` atlas manifest to
//! the generic `ISprite` layer (`crosscom/idl/scripting_services.idl`).
//!
//! This is a reusable **format adapter**, not a per-screen scene object:
//! it resolves a sprite *name* (the logical path from the manifest) to a
//! generic `ISprite` carved from the atlas page that backs it, loading
//! and caching each page once on demand through a generic
//! `ISpriteService`. Any PAL3 screen authored against `UI_opt.tli`
//! composes itself by name through this.

use std::cell::RefCell;
use std::collections::HashMap;

use crosscom::ComRc;
use radiance_scripting::comdef::services::{IAtlasPage, ISprite, ISpriteService};

use super::comdef::{IPal3UiAtlas, IPal3UiAtlasImpl};
use super::loaders::pos;
use super::loaders::tli::TliDict;

/// Native canvas the original PAL3 cover art was authored against.
const NATIVE_WIDTH: i32 = 800;
const NATIVE_HEIGHT: i32 = 600;

/// Path inside the PAL3 vfs to the atlas-index manifest.
const TLI_PATH: &str = "/basedata/basedata/ui/UILib/UI_opt.tli";
/// PAL3A's atlas-index manifest (`UIArtist.plug`), replacing UI_opt.tli.
const PLUG_PATH: &str = "/basedata/basedata/ui/UIArtist.plug";
/// Directory the manifest's atlas-page references resolve against.
const UILIB_DIR: &str = "/basedata/basedata/ui/UILib/";

/// Which on-disk manifest format backs the atlas. PAL3 ships the text
/// `UI_opt.tli`; PAL3A ships the `UIArtist.plug` variant instead.
#[derive(Copy, Clone, PartialEq)]
pub enum AtlasManifest {
    Tli,
    Plug,
}

pub struct Pal3UiAtlas {
    sprites: ComRc<ISpriteService>,
    tli: TliDict,
    /// `lib name (lowercase) -> loaded atlas page` (None once a load
    /// attempt has failed, so we don't retry every lookup).
    pages: RefCell<HashMap<String, Option<ComRc<IAtlasPage>>>>,
}

ComObject_Pal3UiAtlas!(super::Pal3UiAtlas);

impl Pal3UiAtlas {
    /// Build an atlas adapter by parsing `UI_opt.tli` from the PAL3 vfs
    /// reachable through `sprites`, **eagerly loading every distinct
    /// atlas page up front**. Eager loading is essential: the page
    /// upload borrows the shared `ImguiTextureCache`, which the imgui
    /// pump holds borrowed for the whole frame during `render` — so a
    /// lazy load triggered by a per-frame `sprite(name)` call would
    /// panic with "RefCell already borrowed". `create` runs outside any
    /// frame (during director construction), so the uploads are safe
    /// here and per-frame `sprite` lookups never touch the cache.
    pub fn create(
        sprites: ComRc<ISpriteService>,
        manifest_bytes: &[u8],
        kind: AtlasManifest,
    ) -> ComRc<IPal3UiAtlas> {
        let tli = match kind {
            AtlasManifest::Tli => TliDict::parse(manifest_bytes),
            AtlasManifest::Plug => pos::parse(manifest_bytes),
        };
        let mut pages: HashMap<String, Option<ComRc<IAtlasPage>>> = HashMap::new();
        for (lib, _lw, _lh) in tli.distinct_libs() {
            let key = lib.to_lowercase();
            if pages.contains_key(&key) {
                continue;
            }
            pages.insert(key, load_page(&sprites, &lib));
        }
        log::info!(
            "Pal3UiAtlas: loaded atlas manifest ({} entries, {} atlases)",
            tli.len(),
            pages.len()
        );
        ComRc::from_object(Self {
            sprites,
            tli,
            pages: RefCell::new(pages),
        })
    }

    /// The vfs path of the atlas-index manifest for the given format.
    pub fn manifest_path(kind: AtlasManifest) -> &'static str {
        match kind {
            AtlasManifest::Tli => TLI_PATH,
            AtlasManifest::Plug => PLUG_PATH,
        }
    }

    /// Look up an already-loaded atlas page by `lib` name (populated
    /// eagerly in `create`). Returns `None` for an unknown/failed page —
    /// never loads here, so it is safe to call mid-frame.
    fn page_for(&self, lib: &str) -> Option<ComRc<IAtlasPage>> {
        self.pages
            .borrow()
            .get(&lib.to_lowercase())
            .cloned()
            .flatten()
    }
}

/// Load a single atlas page by manifest `lib` name. PAL3 atlases are
/// referenced with a `.tga` extension in the manifest but several ship
/// only as `.dds` on disk, so try the literal name first and fall back
/// to the `.dds` variant. Runs only from `Pal3UiAtlas::create` (outside
/// any imgui frame).
fn load_page(sprites: &ComRc<ISpriteService>, lib: &str) -> Option<ComRc<IAtlasPage>> {
    let primary = format!("{}{}", UILIB_DIR, lib);
    let page = sprites.load_atlas_page(&primary).or_else(|| {
        let lower = primary.to_ascii_lowercase();
        lower
            .strip_suffix(".tga")
            .map(|stem| format!("{}.dds", stem))
            .and_then(|dds| sprites.load_atlas_page(&dds))
    });
    if page.is_none() {
        log::warn!("Pal3UiAtlas: atlas page {} failed to load", primary);
    }
    page
}

impl IPal3UiAtlasImpl for Pal3UiAtlas {
    fn has_sprite(&self, name: &str) -> bool {
        self.tli.get(name).is_some()
    }

    fn sprite(&self, name: &str) -> Option<ComRc<ISprite>> {
        let entry = self.tli.get(name)?;
        let page = self.page_for(&entry.lib)?;
        Some(page.sprite(
            entry.orix as i32,
            entry.oriy as i32,
            entry.w as i32,
            entry.h as i32,
        ))
    }

    fn native_width(&self) -> i32 {
        NATIVE_WIDTH
    }

    fn native_height(&self) -> i32 {
        NATIVE_HEIGHT
    }
}
