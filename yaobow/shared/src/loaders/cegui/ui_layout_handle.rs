//! Runtime + editor handle for a parsed CEGUI `<GUILayout>` XML.
//!
//! Parses the layout, resolves every referenced `.imageset`, uploads
//! each imageset PNG into the imgui texture cache once, and exposes
//! a flat back-to-front draw list plus per-window tree metadata via
//! `IUiLayoutHandle`. The PAL4 start-menu director and the
//! `yaobow_editor` UI-layout previewer both consume the same handle
//! through this single Rust impl.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use mini_fs::{EntryKind, MiniFs, StoreExt};

use crate::loaders::cegui::imageset::{self as cegui_imageset, Imageset};
use crate::loaders::cegui::layout::{self as cegui_layout, LayoutFile, Window};
use radiance_scripting::comdef::services::{IUiLayoutHandle, IUiLayoutHandleImpl};
use radiance_scripting::services::texture_cache::{ImguiTextureCache, next_handle_com_id};

#[derive(Debug, Clone, Copy)]
struct StateImage {
    texture_com_id: i64,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
}

impl StateImage {
    const fn empty() -> Self {
        Self {
            // 0 is the "no texture" sentinel. `next_handle_com_id`
            // starts at -1 and decrements, so a real com_id is never
            // 0; the script renderer uses `tex != 0` as the
            // has-texture predicate.
            texture_com_id: 0,
            u0: 0.0,
            v0: 0.0,
            u1: 0.0,
            v1: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
struct DrawCmd {
    window_id: u32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    /// Default (normal-state) image. Always populated — we never emit
    /// a `DrawCmd` without at least the normal-state image resolved
    /// (or a pure text label).
    base: StateImage,
    /// Optional state imagery for interactive widgets.
    hover: StateImage,
    pressed: StateImage,
    disabled: StateImage,
    /// `true` if any alt state is configured OR the widget type is a
    /// known interactive type (`OiramLook/Button`, etc.). The script
    /// renderer uses this to decide whether to query hover/click for
    /// state-image overdraw.
    is_interactive: bool,
    /// `Text` property if the window had one. May be empty.
    text: String,
}

struct ResolvedSet {
    com_id: i64,
    imageset: Imageset,
    tex_w: u32,
    tex_h: u32,
}

pub struct UiLayoutHandle {
    layout: LayoutFile,
    draws: Vec<DrawCmd>,
    text_dump: String,
    native_w: u32,
    native_h: u32,
    /// Uploaded imageset PNG com_ids, one per `set:` name. Released
    /// on `Drop` via the same pending-forget queue `ImageHandle` uses.
    uploaded_com_ids: Vec<i64>,
    pending_forgets: Rc<RefCell<Vec<i64>>>,
    /// Reusable scratch buffer for `text_dump()`'s `&str` return.
    last_string: RefCell<String>,
    /// Reusable scratch buffer for `window_name`/`window_type` `&str`
    /// returns. Each accessor refreshes the buffer before returning a
    /// pointer into it; codegen copies into a CString immediately.
    name_scratch: RefCell<String>,
    type_scratch: RefCell<String>,
    text_scratch: RefCell<String>,
}

ComObject_UiLayoutHandle!(super::UiLayoutHandle);

impl UiLayoutHandle {
    pub fn try_create(
        vfs: &MiniFs,
        vfs_path: &str,
        cache: Rc<RefCell<ImguiTextureCache>>,
    ) -> Option<ComRc<IUiLayoutHandle>> {
        let layout = match cegui_layout::load_layout(vfs, vfs_path) {
            Ok(l) => l,
            Err(e) => {
                log::warn!("UI layout {} parse failed: {:#}", vfs_path, e);
                return None;
            }
        };
        log::info!(
            "UiLayoutHandle::try_create({}): parsed {} window(s)",
            vfs_path,
            layout.windows.len()
        );

        let imageset_dir = imageset_dir_for(vfs_path);
        log::info!(
            "UiLayoutHandle({}): imageset_dir={}",
            vfs_path,
            imageset_dir
        );
        let mut imageset_index = ImagesetIndex::new(imageset_dir);

        let mut resolved_sets: HashMap<String, ResolvedSet> = HashMap::new();
        let mut uploaded_com_ids: Vec<i64> = Vec::new();

        // First pass: collect every `set:` referenced by any of the
        // state images (Image / olNormal / olHighlight / olPushed /
        // olDisabled) in document order so the upload order is
        // deterministic.
        let mut unique_sets: Vec<String> = Vec::new();
        {
            let mut seen: std::collections::HashSet<String> = Default::default();
            for window in &layout.windows {
                let candidates: [&Option<crate::loaders::cegui::layout::ImageRef>; 5] = [
                    &window.image,
                    &window.overlay_normal_image,
                    &window.overlay_highlight_image,
                    &window.overlay_pushed_image,
                    &window.overlay_disabled_image,
                ];
                for slot in candidates {
                    if let Some(refr) = slot {
                        let key = refr.set.to_lowercase();
                        if seen.insert(key.clone()) {
                            unique_sets.push(refr.set.clone());
                        }
                    }
                }
            }
        }
        log::info!(
            "UiLayoutHandle({}): {} unique imageset reference(s): {:?}",
            vfs_path,
            unique_sets.len(),
            unique_sets
        );

        for set_name in &unique_sets {
            match imageset_index.load(vfs, set_name) {
                Ok((imageset, png_bytes)) => {
                    log::info!(
                        "UiLayoutHandle({}): set:{} resolved to imageset Name='{}' atlas_declared='{}', png_bytes={}",
                        vfs_path,
                        set_name,
                        imageset.name,
                        imageset.image_path,
                        png_bytes.len()
                    );
                    let dyn_image = match image::load_from_memory(&png_bytes) {
                        Ok(img) => img,
                        Err(e) => {
                            log::warn!(
                                "imageset {} PNG decode failed for layout {}: {:#}",
                                set_name,
                                vfs_path,
                                e
                            );
                            continue;
                        }
                    };
                    let rgba = dyn_image.to_rgba8();
                    let (tex_w, tex_h) = rgba.dimensions();
                    let com_id = next_handle_com_id();
                    {
                        let mut c = cache.borrow_mut();
                        if c.upload_pixels(com_id, &rgba.into_raw(), tex_w, tex_h)
                            .is_none()
                        {
                            log::warn!(
                                "imageset {} texture upload failed for layout {}",
                                set_name,
                                vfs_path
                            );
                            continue;
                        }
                    }
                    log::info!(
                        "UiLayoutHandle({}): set:{} uploaded as com_id={} ({}x{})",
                        vfs_path,
                        set_name,
                        com_id,
                        tex_w,
                        tex_h
                    );
                    uploaded_com_ids.push(com_id);
                    resolved_sets.insert(
                        set_name.to_lowercase(),
                        ResolvedSet {
                            com_id,
                            imageset,
                            tex_w,
                            tex_h,
                        },
                    );
                }
                Err(e) => {
                    log::warn!(
                        "UiLayoutHandle({}): set:{} not found: {:#}",
                        vfs_path,
                        set_name,
                        e
                    );
                }
            }
        }

        // Determine native canvas size: prefer the first resolved
        // imageset's native res (CEGUI layout coordinates are
        // expressed in the imageset's native canvas); fall back to
        // 800x600.
        let (native_w, native_h) = resolved_sets
            .values()
            .next()
            .map(|s| (s.imageset.native_horz_res, s.imageset.native_vert_res))
            .unwrap_or((800, 600));

        // Pre-order walk that builds resolved (x,y,w,h) per window
        // against the inherited parent area, while emitting draw cmds
        // for any window with an `Image` property.
        let mut draws: Vec<DrawCmd> = Vec::new();
        if let Some(root) = layout.root() {
            let (rx, ry, rw, rh) = root.area.resolve(native_w as f32, native_h as f32);
            let rw = if rw > 0.0 { rw } else { native_w as f32 };
            let rh = if rh > 0.0 { rh } else { native_h as f32 };
            emit_draws(root, rx, ry, rw, rh, &layout, &resolved_sets, &mut draws);
        }
        log::info!(
            "UiLayoutHandle({}): produced {} draw cmd(s), native={}x{}",
            vfs_path,
            draws.len(),
            native_w,
            native_h
        );

        let text_dump = serde_json::to_string_pretty(&layout)
            .unwrap_or_else(|_| "Cannot serialize layout into JSON".to_string());

        let pending_forgets = cache.borrow().pending_forgets_sink();

        Some(ComRc::from_object(Self {
            layout,
            draws,
            text_dump,
            native_w,
            native_h,
            uploaded_com_ids,
            pending_forgets,
            last_string: RefCell::new(String::new()),
            name_scratch: RefCell::new(String::new()),
            type_scratch: RefCell::new(String::new()),
            text_scratch: RefCell::new(String::new()),
        }))
    }
}

impl Drop for UiLayoutHandle {
    fn drop(&mut self) {
        // Mirror `ImageHandle::Drop`: queue every uploaded texture
        // com_id for cache removal so we never `borrow_mut()` the
        // cache while it's held by an active imgui frame.
        let mut q = self.pending_forgets.borrow_mut();
        for id in self.uploaded_com_ids.drain(..) {
            q.push(id);
        }
    }
}

impl IUiLayoutHandleImpl for UiLayoutHandle {
    fn native_width(&self) -> i32 {
        self.native_w as i32
    }
    fn native_height(&self) -> i32 {
        self.native_h as i32
    }
    fn text_dump(&self) -> &str {
        *self.last_string.borrow_mut() = self.text_dump.clone();
        // SAFETY: single-threaded script/UI path; codegen copies the
        // &str into a CString immediately.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }
    fn draw_count(&self) -> i32 {
        self.draws.len() as i32
    }
    fn draw_window_id(&self, i: i32) -> i32 {
        self.draws
            .get(i as usize)
            .map(|d| d.window_id as i32)
            .unwrap_or(-1)
    }
    fn draw_x(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.x).unwrap_or(0.0)
    }
    fn draw_y(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.y).unwrap_or(0.0)
    }
    fn draw_w(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.w).unwrap_or(0.0)
    }
    fn draw_h(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.h).unwrap_or(0.0)
    }
    fn draw_texture_com_id(&self, i: i32) -> i32 {
        self.draws
            .get(i as usize)
            .map(|d| d.base.texture_com_id as i32)
            .unwrap_or(0)
    }
    fn draw_u0(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.base.u0).unwrap_or(0.0)
    }
    fn draw_v0(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.base.v0).unwrap_or(0.0)
    }
    fn draw_u1(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.base.u1).unwrap_or(1.0)
    }
    fn draw_v1(&self, i: i32) -> f32 {
        self.draws.get(i as usize).map(|d| d.base.v1).unwrap_or(1.0)
    }

    fn draw_hover_texture_com_id(&self, i: i32) -> i32 {
        self.draws
            .get(i as usize)
            .map(|d| d.hover.texture_com_id as i32)
            .unwrap_or(0)
    }
    fn draw_hover_u0(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.hover.u0)
            .unwrap_or(0.0)
    }
    fn draw_hover_v0(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.hover.v0)
            .unwrap_or(0.0)
    }
    fn draw_hover_u1(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.hover.u1)
            .unwrap_or(1.0)
    }
    fn draw_hover_v1(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.hover.v1)
            .unwrap_or(1.0)
    }
    fn draw_pressed_texture_com_id(&self, i: i32) -> i32 {
        self.draws
            .get(i as usize)
            .map(|d| d.pressed.texture_com_id as i32)
            .unwrap_or(0)
    }
    fn draw_pressed_u0(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.pressed.u0)
            .unwrap_or(0.0)
    }
    fn draw_pressed_v0(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.pressed.v0)
            .unwrap_or(0.0)
    }
    fn draw_pressed_u1(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.pressed.u1)
            .unwrap_or(1.0)
    }
    fn draw_pressed_v1(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.pressed.v1)
            .unwrap_or(1.0)
    }
    fn draw_disabled_texture_com_id(&self, i: i32) -> i32 {
        self.draws
            .get(i as usize)
            .map(|d| d.disabled.texture_com_id as i32)
            .unwrap_or(0)
    }
    fn draw_disabled_u0(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.disabled.u0)
            .unwrap_or(0.0)
    }
    fn draw_disabled_v0(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.disabled.v0)
            .unwrap_or(0.0)
    }
    fn draw_disabled_u1(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.disabled.u1)
            .unwrap_or(1.0)
    }
    fn draw_disabled_v1(&self, i: i32) -> f32 {
        self.draws
            .get(i as usize)
            .map(|d| d.disabled.v1)
            .unwrap_or(1.0)
    }
    fn draw_is_interactive(&self, i: i32) -> bool {
        self.draws
            .get(i as usize)
            .map(|d| d.is_interactive)
            .unwrap_or(false)
    }
    fn draw_text(&self, i: i32) -> &str {
        let mut buf = self.text_scratch.borrow_mut();
        *buf = self
            .draws
            .get(i as usize)
            .map(|d| d.text.clone())
            .unwrap_or_default();
        drop(buf);
        // SAFETY: see text_dump.
        unsafe { (*self.text_scratch.as_ptr()).as_str() }
    }
    fn draw_has_text(&self, i: i32) -> bool {
        self.draws
            .get(i as usize)
            .map(|d| !d.text.is_empty())
            .unwrap_or(false)
    }

    fn window_count(&self) -> i32 {
        self.layout.windows.len() as i32
    }
    fn window_name(&self, i: i32) -> &str {
        let mut buf = self.name_scratch.borrow_mut();
        *buf = self
            .layout
            .windows
            .get(i as usize)
            .map(|w| w.name.clone())
            .unwrap_or_default();
        drop(buf);
        unsafe { (*self.name_scratch.as_ptr()).as_str() }
    }
    fn window_type(&self, i: i32) -> &str {
        let mut buf = self.type_scratch.borrow_mut();
        *buf = self
            .layout
            .windows
            .get(i as usize)
            .map(|w| w.window_type.clone())
            .unwrap_or_default();
        drop(buf);
        unsafe { (*self.type_scratch.as_ptr()).as_str() }
    }
}

/// Recursive walker that turns the layout tree into a flat draw list.
fn emit_draws(
    window: &Window,
    px: f32,
    py: f32,
    pw: f32,
    ph: f32,
    layout: &LayoutFile,
    resolved_sets: &HashMap<String, ResolvedSet>,
    draws: &mut Vec<DrawCmd>,
) {
    if !window.visible {
        return;
    }

    let (rel_x, rel_y, rw, rh) = window.area.resolve(pw, ph);
    let ax = px + rel_x;
    let ay = py + rel_y;
    let aw = if rw > 0.0 { rw } else { pw };
    let ah = if rh > 0.0 { rh } else { ph };

    let base = window
        .image
        .as_ref()
        .and_then(|r| resolve_state_image(r, resolved_sets))
        .or_else(|| {
            window
                .overlay_normal_image
                .as_ref()
                .and_then(|r| resolve_state_image(r, resolved_sets))
        });

    let hover = window
        .overlay_highlight_image
        .as_ref()
        .and_then(|r| resolve_state_image(r, resolved_sets))
        .unwrap_or(StateImage::empty());
    let pressed = window
        .overlay_pushed_image
        .as_ref()
        .and_then(|r| resolve_state_image(r, resolved_sets))
        .unwrap_or(StateImage::empty());
    let disabled = window
        .overlay_disabled_image
        .as_ref()
        .and_then(|r| resolve_state_image(r, resolved_sets))
        .unwrap_or(StateImage::empty());

    if let Some(base) = base {
        let is_interactive = is_interactive_widget(&window.window_type)
            || hover.texture_com_id != 0
            || pressed.texture_com_id != 0;
        draws.push(DrawCmd {
            window_id: window.id,
            x: ax,
            y: ay,
            w: aw,
            h: ah,
            base,
            hover,
            pressed,
            disabled,
            is_interactive,
            text: window.text.clone().unwrap_or_default(),
        });
    } else if window.text.is_some() && !window.text.as_deref().unwrap_or("").is_empty() {
        draws.push(DrawCmd {
            window_id: window.id,
            x: ax,
            y: ay,
            w: aw,
            h: ah,
            base: StateImage::empty(),
            hover: StateImage::empty(),
            pressed: StateImage::empty(),
            disabled: StateImage::empty(),
            is_interactive: false,
            text: window.text.clone().unwrap_or_default(),
        });
    }

    // Recurse: non-AlwaysOnTop children first, then on-top children.
    let mut on_top: Vec<u32> = Vec::new();
    for &child_id in &window.children {
        let Some(child) = layout.windows.get(child_id as usize) else {
            continue;
        };
        if child.always_on_top {
            on_top.push(child_id);
        } else {
            emit_draws(child, ax, ay, aw, ah, layout, resolved_sets, draws);
        }
    }
    for child_id in on_top {
        let Some(child) = layout.windows.get(child_id as usize) else {
            continue;
        };
        emit_draws(child, ax, ay, aw, ah, layout, resolved_sets, draws);
    }
}

fn resolve_state_image(
    refr: &crate::loaders::cegui::layout::ImageRef,
    resolved_sets: &HashMap<String, ResolvedSet>,
) -> Option<StateImage> {
    let set = resolved_sets.get(&refr.set.to_lowercase())?;
    let rect = set.imageset.get(&refr.image)?;
    let (u0, v0, u1, v1) = uv_for(rect, set.tex_w, set.tex_h);
    Some(StateImage {
        texture_com_id: set.com_id,
        u0,
        v0,
        u1,
        v1,
    })
}

fn is_interactive_widget(window_type: &str) -> bool {
    matches!(
        window_type,
        "OiramLook/Button"
            | "OiramLook/Checkbox"
            | "OiramLook/RadioButton"
            | "OiramLook/MenuItem"
            | "OiramLook/PopupMenuItem"
    )
}

fn uv_for(rect: &cegui_imageset::ImagesetImage, tex_w: u32, tex_h: u32) -> (f32, f32, f32, f32) {
    let tw = tex_w.max(1) as f32;
    let th = tex_h.max(1) as f32;
    // Half-texel inset: shrink the sampled rectangle by half a
    // texel on each side so a LINEAR filter never blends with the
    // neighbouring atlas cell at the quad edge. Without this every
    // adjacent CEGUI tile shows a faint 1-pixel colour seam.
    //
    // For degenerate 1-texel-wide / -tall sprites (width<2 or
    // height<2) the inset would invert u0>u1; in that case sample
    // the single texel's centre on that axis instead.
    let (u0, u1) = if rect.width >= 2 {
        let ix = 0.5 / tw;
        (
            rect.x as f32 / tw + ix,
            (rect.x + rect.width) as f32 / tw - ix,
        )
    } else {
        let c = (rect.x as f32 + 0.5) / tw;
        (c, c)
    };
    let (v0, v1) = if rect.height >= 2 {
        let iy = 0.5 / th;
        (
            rect.y as f32 / th + iy,
            (rect.y + rect.height) as f32 / th - iy,
        )
    } else {
        let c = (rect.y as f32 + 0.5) / th;
        (c, c)
    };
    (u0, v0, u1, v1)
}

/// Compute the imageset directory paired with a layout file's path.
/// PAL4's `ui.cpk` mounts at `gamedata/ui/`, so layouts live at
/// `gamedata/ui/layouts/X.xml` and their `.imageset` siblings at
/// `gamedata/ui/imagesets/X.imageset`. The general rule is "replace
/// the layout's `layouts` segment with `imagesets`".
fn imageset_dir_for(layout_path: &str) -> String {
    let norm = layout_path.replace('\\', "/");
    let dir = match norm.rfind('/') {
        Some(idx) => &norm[..idx],
        None => "/",
    };
    let trimmed = if dir.to_ascii_lowercase().ends_with("/layouts") {
        &dir[..dir.len() - "/layouts".len()]
    } else {
        dir
    };
    if trimmed.is_empty() {
        "/imagesets".to_string()
    } else {
        format!("{}/imagesets", trimmed)
    }
}

/// Lazy index over `*.imageset` files in a directory. On first miss
/// we scan the directory and build an index keyed by the imageset's
/// internal `Name` attribute (case-insensitive). The two cheap
/// filename-based candidates (`{set}.imageset`, `{set}_0.imageset`)
/// are tried first to avoid the dir scan for the common case.
struct ImagesetIndex {
    dir: String,
    name_to_path: HashMap<String, String>,
    scanned: bool,
}

impl ImagesetIndex {
    fn new(dir: String) -> Self {
        Self {
            dir,
            name_to_path: HashMap::new(),
            scanned: false,
        }
    }

    fn join(&self, name: &str) -> String {
        if self.dir.ends_with('/') {
            format!("{}{}", self.dir, name)
        } else {
            format!("{}/{}", self.dir, name)
        }
    }

    fn load(&mut self, vfs: &MiniFs, set_name: &str) -> anyhow::Result<(Imageset, Vec<u8>)> {
        let lower = set_name.to_lowercase();
        let candidates = [
            format!("{}.imageset", set_name),
            format!("{}_0.imageset", set_name),
        ];
        for cand in &candidates {
            let p = self.join(cand);
            match vfs.read_to_end(&p) {
                Ok(b) => {
                    if let Ok(imageset) = cegui_imageset::parse_imageset_bytes(&b) {
                        if imageset.name.eq_ignore_ascii_case(set_name) {
                            let png = cegui_imageset::read_atlas_png(vfs, &p, &imageset)?;
                            return Ok((imageset, png));
                        }
                    }
                }
                Err(_) => {}
            }
        }
        if !self.scanned {
            self.scan(vfs);
            self.scanned = true;
        }
        if let Some(p) = self.name_to_path.get(&lower) {
            let (imageset, png) = cegui_imageset::load_imageset_with_atlas(vfs, p)?;
            return Ok((imageset, png));
        }
        Err(anyhow::anyhow!(
            "no .imageset file declares Name='{}' under {}",
            set_name,
            self.dir
        ))
    }

    fn scan(&mut self, vfs: &MiniFs) {
        let entries = match vfs.entries(&self.dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            if !matches!(entry.kind, EntryKind::File) {
                continue;
            }
            let entry_name = std::path::Path::new(&entry.name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if !entry_name.to_lowercase().ends_with(".imageset") {
                continue;
            }
            let path = self.join(entry_name);
            let Ok(bytes) = vfs.read_to_end(&path) else {
                continue;
            };
            let Ok(imageset) = cegui_imageset::parse_imageset_bytes(&bytes) else {
                continue;
            };
            self.name_to_path.insert(imageset.name.to_lowercase(), path);
        }
    }
}
