//! `Pal3StartMenuScene` — sprite-atlas + BGM source for the PAL3 start
//! menu, exposed to p7 via `IPal3StartMenuScene`.
//!
//! The PAL3 game data ships exactly one UI manifest
//! (`ui\\UILib\\UI_opt.tli`) and a set of atlas pages
//! (`ui\\UILib\\1..11.tga|.dds`). There are no layout files — the
//! original `pal3.dll` hard-codes screen positions. So this scene
//! exposes only the sprite catalogue (atlas com_id + UV sub-rect +
//! native sub-rect size) and the menu BGM (`PI01`); the on-screen
//! layout is authored entirely in p7.
//!
//! On construction we:
//!   1. Parse `UI_opt.tli` via `loaders::tli::TliDict`.
//!   2. Decode each distinct atlas page (`image::load_from_memory`,
//!      with a TGA-format fallback to match the rest of the loaders
//!      in this crate). Default `image` features include both `tga`
//!      and `dds`, so both atlas variants are accepted.
//!   3. Upload every successfully-decoded page to the shared
//!      `ImguiTextureCache` under a fresh `next_handle_com_id()`,
//!      remembering the com_id so we can release it on `Drop` via the
//!      cache's pending-forget queue (same pattern as
//!      `UiLayoutHandle`).
//!   4. Pre-load the `PI01` MP3 BGM and mint a fresh
//!      `AudioMemorySource`; `play_bgm` starts it looping,
//!      `update_bgm` is driven from the menu director's `render_im`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::audio::{AudioEngine, AudioMemorySource, AudioSourceState, Codec};
use radiance_scripting::services::texture_cache::{ImguiTextureCache, next_handle_com_id};

use super::asset_manager::AssetManager;
use super::comdef::{IPal3StartMenuScene, IPal3StartMenuSceneImpl};
use super::loaders::tli::{TliDict, TliEntry};

/// Native canvas the original PAL3 cover art was authored against
/// (the menu background atlas tile is 800×600).
const NATIVE_WIDTH: i32 = 800;
const NATIVE_HEIGHT: i32 = 600;

/// Path inside the PAL3 vfs to the atlas-index manifest.
const TLI_PATH: &str = "/basedata/basedata/ui/UILib/UI_opt.tli";
/// Directory the manifest's `t_lib` references resolve against.
const UILIB_DIR: &str = "/basedata/basedata/ui/UILib/";

#[derive(Debug, Clone, Copy)]
struct ResolvedSprite {
    com_id: i64,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    w: i32,
    h: i32,
}

pub struct Pal3StartMenuScene {
    /// `canonical key -> resolved sprite`. Keys match
    /// `TliDict::get`'s canonical form (lowercase, `/`-separated).
    sprites: HashMap<String, ResolvedSprite>,
    /// All atlas com_ids we uploaded, drained into the cache's
    /// pending-forget queue on `Drop`.
    uploaded_com_ids: Vec<i64>,
    pending_forgets: Rc<RefCell<Vec<i64>>>,
    bgm: RefCell<Option<Box<dyn AudioMemorySource>>>,
}

ComObject_Pal3StartMenuScene!(super::Pal3StartMenuScene);

impl Pal3StartMenuScene {
    /// Build a scene against the given PAL3 `AssetManager` (mounted
    /// vfs + component factory) and the shared imgui texture cache.
    /// `audio_engine` is used to mint the BGM source up front.
    ///
    /// Returns `None` when the TLI manifest cannot be read; otherwise
    /// always returns a usable scene — individual sprite or atlas
    /// failures are logged and degrade to `has_sprite` reporting
    /// false / `sprite_com_id` returning 0.
    pub fn create(
        asset_mgr: Rc<AssetManager>,
        audio_engine: Rc<dyn AudioEngine>,
        cache: Rc<RefCell<ImguiTextureCache>>,
    ) -> Option<ComRc<IPal3StartMenuScene>> {
        let vfs = asset_mgr.vfs();
        let tli_bytes = match common::store_ext::StoreExt2::read_to_end(vfs, TLI_PATH) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Pal3StartMenuScene: cannot read {}: {:#}", TLI_PATH, e);
                return None;
            }
        };
        let dict = TliDict::parse(&tli_bytes);
        log::info!(
            "Pal3StartMenuScene: loaded UI_opt.tli ({} entries, {} atlases)",
            dict.len(),
            dict.distinct_libs().len()
        );

        let pending_forgets = cache.borrow().pending_forgets_sink();
        let mut uploaded_com_ids: Vec<i64> = Vec::new();
        let mut atlas_com_ids: HashMap<String, i64> = HashMap::new();

        for (lib_name, _lw, _lh) in dict.distinct_libs() {
            // The TLI always names atlases with a `.tga` extension,
            // but several PAL3 atlases ship only as `.dds` on disk
            // (e.g. `3.tga` → file `3.dds`). Try the literal name
            // first; on miss, swap to `.dds`.
            let primary_path = format!("{}{}", UILIB_DIR, lib_name);
            let dds_path = dds_variant_path(&primary_path);
            let (load_path, bytes) = match common::store_ext::StoreExt2::read_to_end(
                vfs,
                &primary_path,
            ) {
                Ok(b) => (primary_path.clone(), b),
                Err(_) => match dds_path.as_ref().and_then(|p| {
                    common::store_ext::StoreExt2::read_to_end(vfs, p.as_str())
                        .ok()
                        .map(|b| (p.clone(), b))
                }) {
                    Some(found) => found,
                    None => {
                        log::warn!(
                            "Pal3StartMenuScene: atlas {} unreadable (no .tga, no .dds); skipping",
                            primary_path
                        );
                        continue;
                    }
                },
            };
            let decoded = image::load_from_memory(&bytes)
                .or_else(|_| image::load_from_memory_with_format(&bytes, image::ImageFormat::Tga));
            let dyn_image = match decoded {
                Ok(img) => img,
                Err(e) => {
                    log::warn!(
                        "Pal3StartMenuScene: atlas {} decode failed: {:#}",
                        load_path,
                        e
                    );
                    continue;
                }
            };
            // PAL3's `.dds` atlases were saved in the D3D9 bottom-up
            // convention (origin at the lower-left), but the `image`
            // crate's DXT decoder feeds bytes top-down. Without a flip
            // every DDS atlas reads upside down — which manifests as
            // sprites rendering inverted (and, since the dial is roughly
            // symmetric, looks like a 180° rotation). TGA files carry
            // their origin in the image-descriptor byte and are
            // flipped correctly by the decoder, so this fix-up runs
            // only for the `.dds` fallback path.
            let dyn_image = if load_path.to_ascii_lowercase().ends_with(".dds") {
                dyn_image.flipv()
            } else {
                dyn_image
            };
            let rgba = dyn_image.to_rgba8();
            let (tex_w, tex_h) = rgba.dimensions();
            let com_id = next_handle_com_id();
            let uploaded = {
                let mut c = cache.borrow_mut();
                c.upload_pixels(com_id, &rgba.into_raw(), tex_w, tex_h)
            };
            if uploaded.is_none() {
                log::warn!(
                    "Pal3StartMenuScene: atlas {} upload failed (com_id={})",
                    load_path,
                    com_id
                );
                continue;
            }
            log::info!(
                "Pal3StartMenuScene: atlas {} uploaded as com_id={} ({}x{})",
                load_path,
                com_id,
                tex_w,
                tex_h
            );
            uploaded_com_ids.push(com_id);
            atlas_com_ids.insert(lib_name.to_lowercase(), com_id);
        }

        let mut sprites: HashMap<String, ResolvedSprite> = HashMap::new();
        for (key, entry) in dict.iter() {
            let Some(&com_id) = atlas_com_ids.get(&entry.lib.to_lowercase()) else {
                continue;
            };
            sprites.insert(key.clone(), resolve_sprite(entry, com_id));
        }
        log::info!(
            "Pal3StartMenuScene: {} sprites resolved across {} atlases",
            sprites.len(),
            atlas_com_ids.len()
        );

        // Pre-load PI01 (menu BGM). Failure is non-fatal — the menu
        // still renders, just without music.
        let bgm = {
            let data = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                asset_mgr.load_music_data("PI01")
            }));
            match data {
                Ok(bytes) => {
                    let mut src = audio_engine.create_source();
                    src.set_data(bytes, Codec::Mp3);
                    Some(src)
                }
                Err(_) => {
                    log::warn!("Pal3StartMenuScene: PI01 BGM missing");
                    None
                }
            }
        };

        Some(ComRc::from_object(Self {
            sprites,
            uploaded_com_ids,
            pending_forgets,
            bgm: RefCell::new(bgm),
        }))
    }
}

fn resolve_sprite(entry: &TliEntry, com_id: i64) -> ResolvedSprite {
    // Inset each UV edge by half a texel to prevent atlas bleeding.
    // Without this, the GPU's bilinear sampler at the exact sprite
    // boundary picks up neighbouring atlas pixels — manifests as
    // thin white/coloured seams on the right or bottom edge of e.g.
    // some yun clouds where the adjacent atlas slot is opaque.
    //
    // 0.5 / atlas_dim is the classical half-texel correction; tiny
    // sprites still survive because the inset is in UV space, not
    // pixels.
    let lw = entry.lib_w.max(1) as f32;
    let lh = entry.lib_h.max(1) as f32;
    let inset_u = 0.5 / lw;
    let inset_v = 0.5 / lh;
    let u0 = entry.orix as f32 / lw + inset_u;
    let v0 = entry.oriy as f32 / lh + inset_v;
    let u1 = (entry.orix + entry.w) as f32 / lw - inset_u;
    let v1 = (entry.oriy + entry.h) as f32 / lh - inset_v;
    ResolvedSprite {
        com_id,
        u0,
        v0,
        u1,
        v1,
        w: entry.w as i32,
        h: entry.h as i32,
    }
}

fn canonical(name: &str) -> String {
    name.replace('\\', "/").to_lowercase()
}

fn dds_variant_path(tga_path: &str) -> Option<String> {
    let lower = tga_path.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix(".tga") {
        Some(format!("{}.dds", stem))
    } else {
        None
    }
}

impl Drop for Pal3StartMenuScene {
    fn drop(&mut self) {
        let mut q = self.pending_forgets.borrow_mut();
        for id in self.uploaded_com_ids.drain(..) {
            q.push(id);
        }
    }
}

impl IPal3StartMenuSceneImpl for Pal3StartMenuScene {
    fn has_sprite(&self, name: &str) -> bool {
        self.sprites.contains_key(&canonical(name))
    }

    fn sprite_com_id(&self, name: &str) -> i32 {
        self.sprites
            .get(&canonical(name))
            .map(|s| s.com_id as i32)
            .unwrap_or(0)
    }

    fn sprite_u0(&self, name: &str) -> f32 {
        self.sprites
            .get(&canonical(name))
            .map(|s| s.u0)
            .unwrap_or(0.0)
    }
    fn sprite_v0(&self, name: &str) -> f32 {
        self.sprites
            .get(&canonical(name))
            .map(|s| s.v0)
            .unwrap_or(0.0)
    }
    fn sprite_u1(&self, name: &str) -> f32 {
        self.sprites
            .get(&canonical(name))
            .map(|s| s.u1)
            .unwrap_or(0.0)
    }
    fn sprite_v1(&self, name: &str) -> f32 {
        self.sprites
            .get(&canonical(name))
            .map(|s| s.v1)
            .unwrap_or(0.0)
    }

    fn sprite_w(&self, name: &str) -> i32 {
        self.sprites.get(&canonical(name)).map(|s| s.w).unwrap_or(0)
    }
    fn sprite_h(&self, name: &str) -> i32 {
        self.sprites.get(&canonical(name)).map(|s| s.h).unwrap_or(0)
    }

    fn native_width(&self) -> i32 {
        NATIVE_WIDTH
    }
    fn native_height(&self) -> i32 {
        NATIVE_HEIGHT
    }

    fn play_bgm(&self) {
        if let Some(src) = self.bgm.borrow_mut().as_mut() {
            // Idempotent — restart if already playing, kick off if not.
            if matches!(src.state(), AudioSourceState::Playing) {
                src.restart();
            } else {
                src.play(true);
            }
        }
    }

    fn stop_bgm(&self) {
        if let Some(src) = self.bgm.borrow_mut().as_mut() {
            src.stop();
        }
    }

    fn update_bgm(&self) {
        if let Some(src) = self.bgm.borrow_mut().as_mut() {
            src.update();
        }
    }
}
