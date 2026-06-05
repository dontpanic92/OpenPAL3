//! Foreign-handle COM objects exposed to p7 by `IPreviewerHub`.
//!
//! Each handle wraps a chunk of host-owned state (decoded image pixels, an
//! audio source, a video player, a model entity + scene) and exposes a small
//! interface that scripts can call without ever touching raw Rust state.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use common::store_ext::StoreExt2;
use crosscom::ComRc;
use mini_fs::{EntryKind, MiniFs, StoreExt};
use radiance::audio::{AudioMemorySource, AudioSourceState};
use radiance::comdef::IEntity;
use radiance::math::Vec3;
use radiance::rendering::{ComponentFactory, VideoPlayer};
use radiance::scene::CoreScene;
use radiance::video::VideoStreamState;
use radiance_scripting::comdef::services::IRenderTarget;
use radiance_scripting::services::texture_cache::next_handle_com_id;
use radiance_scripting::services::{ImguiTextureCache, ScriptedRenderTarget};
use shared::loaders::cegui::imageset::{self as cegui_imageset, Imageset};
use shared::loaders::cegui::layout::{self as cegui_layout, LayoutFile, Window};

use crate::comdef::editor_services::{
    IAudioHandle, IAudioHandleImpl, IImageHandle, IImageHandleImpl, IModelHandle, IModelHandleImpl,
    IPreviewSession, IPreviewSessionImpl, IUiLayoutHandle, IUiLayoutHandleImpl, IVideoHandle,
    IVideoHandleImpl,
};
use crate::services::preview_registry::{OrbitState, PreviewRegistry, PreviewState, tick_orbit};

// ---------------------------------------------------------------------------
// Image
// ---------------------------------------------------------------------------

pub struct ImageHandle {
    width: u32,
    height: u32,
    com_id: i64,
    /// Independent queue used by `Drop` so cache cleanup never tries to
    /// `borrow_mut()` the cache while it's held by an active imgui
    /// frame (`ImguiImmediateDirectorPump::pump`). See
    /// `ImguiTextureCache::pending_forgets_sink` for details.
    pending_forgets: Rc<RefCell<Vec<i64>>>,
}

ComObject_ImageHandle!(super::ImageHandle);

impl ImageHandle {
    pub fn create(
        cache: Rc<RefCell<ImguiTextureCache>>,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Option<ComRc<IImageHandle>> {
        let com_id = next_handle_com_id();
        let pending_forgets = {
            let mut c = cache.borrow_mut();
            c.upload_pixels(com_id, pixels, width, height)?;
            c.pending_forgets_sink()
        };
        Some(ComRc::from_object(Self {
            width,
            height,
            com_id,
            pending_forgets,
        }))
    }
}

impl Drop for ImageHandle {
    fn drop(&mut self) {
        // Queue the com_id for cache removal; the cache drains its
        // queue on the next `&mut self` access. We can't call
        // `cache.borrow_mut().forget(...)` directly because GC can
        // drop this handle mid-imgui-frame while the cache is held in
        // `borrow_mut()` by the frame state.
        self.pending_forgets.borrow_mut().push(self.com_id);
    }
}

impl IImageHandleImpl for ImageHandle {
    fn width(&self) -> i32 {
        self.width as i32
    }
    fn height(&self) -> i32 {
        self.height as i32
    }
    fn texture_com_id(&self) -> i32 {
        self.com_id as i32
    }
}

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

pub struct AudioHandle {
    source: RefCell<Box<dyn AudioMemorySource>>,
}

ComObject_AudioHandle!(super::AudioHandle);

impl AudioHandle {
    pub fn create(mut source: Box<dyn AudioMemorySource>) -> ComRc<IAudioHandle> {
        source.play(true);
        ComRc::from_object(Self {
            source: RefCell::new(source),
        })
    }
}

impl IAudioHandleImpl for AudioHandle {
    fn toggle(&self) {
        let mut source = self.source.borrow_mut();
        if source.state() == AudioSourceState::Playing {
            source.pause();
        } else {
            source.resume();
        }
    }

    fn state(&self) -> i32 {
        match self.source.borrow().state() {
            AudioSourceState::Stopped => 0,
            AudioSourceState::Playing => 1,
            AudioSourceState::Paused => 2,
        }
    }

    fn update(&self) {
        self.source.borrow_mut().update();
    }
}

// ---------------------------------------------------------------------------
// Video
// ---------------------------------------------------------------------------

pub struct VideoHandle {
    player: RefCell<Box<VideoPlayer>>,
    width: u32,
    height: u32,
    com_id: i64,
    cache: Rc<RefCell<ImguiTextureCache>>,
    pending_forgets: Rc<RefCell<Vec<i64>>>,
    last_texture: RefCell<Option<imgui::TextureId>>,
}

ComObject_VideoHandle!(super::VideoHandle);

impl VideoHandle {
    pub fn create(
        cache: Rc<RefCell<ImguiTextureCache>>,
        player: Box<VideoPlayer>,
        width: u32,
        height: u32,
    ) -> ComRc<IVideoHandle> {
        let com_id = next_handle_com_id();
        let pending_forgets = cache.borrow().pending_forgets_sink();
        ComRc::from_object(Self {
            player: RefCell::new(player),
            width,
            height,
            com_id,
            cache,
            pending_forgets,
            last_texture: RefCell::new(None),
        })
    }
}

impl Drop for VideoHandle {
    fn drop(&mut self) {
        // See `ImageHandle::Drop` — queue the cache removal because
        // GC may fire this destructor mid-imgui-frame while the cache
        // is held in `borrow_mut()` by the frame state.
        self.pending_forgets.borrow_mut().push(self.com_id);
    }
}

impl IVideoHandleImpl for VideoHandle {
    fn toggle(&self) {
        let mut player = self.player.borrow_mut();
        if player.get_state() == VideoStreamState::Playing {
            player.pause();
        } else {
            player.resume();
        }
    }

    fn state(&self) -> i32 {
        match self.player.borrow().get_state() {
            VideoStreamState::Stopped => 0,
            VideoStreamState::Playing => 1,
            VideoStreamState::Paused => 2,
        }
    }

    fn width(&self) -> i32 {
        self.width as i32
    }

    fn height(&self) -> i32 {
        self.height as i32
    }

    fn texture_com_id(&self) -> i32 {
        // Pull a fresh texture id from the player every frame and rebind
        // the cache mapping. The player owns the underlying GPU texture; the
        // cache only stores the id.
        let mut last = self.last_texture.borrow_mut();
        if let Some(id) = self.player.borrow_mut().get_texture(*last) {
            *last = Some(id);
            self.cache.borrow_mut().set_external(self.com_id, id);
        }
        self.com_id as i32
    }
}

// ---------------------------------------------------------------------------
// Model + Preview session
// ---------------------------------------------------------------------------

pub struct PreviewSession {
    state: Rc<PreviewState>,
    target_com: ComRc<IRenderTarget>,
}

ComObject_PreviewSession!(super::PreviewSession);

impl PreviewSession {
    pub fn create(
        state: Rc<PreviewState>,
        target_com: ComRc<IRenderTarget>,
    ) -> ComRc<IPreviewSession> {
        ComRc::from_object(Self { state, target_com })
    }
}

impl Drop for PreviewSession {
    fn drop(&mut self) {
        self.state.closed.set(true);
    }
}

impl IPreviewSessionImpl for PreviewSession {
    fn close(&self) {
        self.state.closed.set(true);
    }

    fn target(&self) -> ComRc<IRenderTarget> {
        self.target_com.clone()
    }

    fn tick_camera(&self, dx: f32, dy: f32, wheel: f32, buttons: i32) {
        if self.state.closed.get() {
            return;
        }
        let mut orbit = self.state.orbit.borrow_mut();
        tick_orbit(&mut orbit, dx, dy, wheel, buttons);
    }
}

pub struct ModelHandle {
    text_dump: String,
    entity: RefCell<Option<ComRc<IEntity>>>,
    factory: Rc<dyn ComponentFactory>,
    cache: Rc<RefCell<ImguiTextureCache>>,
    registry: Rc<PreviewRegistry>,
    last_string: RefCell<String>,
    glb_exporter: Option<Box<dyn Fn() -> anyhow::Result<Vec<u8>>>>,
}

ComObject_ModelHandle!(super::ModelHandle);

impl ModelHandle {
    pub fn create(
        text_dump: String,
        entity: ComRc<IEntity>,
        factory: Rc<dyn ComponentFactory>,
        cache: Rc<RefCell<ImguiTextureCache>>,
        registry: Rc<PreviewRegistry>,
        glb_exporter: Option<Box<dyn Fn() -> anyhow::Result<Vec<u8>>>>,
    ) -> ComRc<IModelHandle> {
        ComRc::from_object(Self {
            text_dump,
            entity: RefCell::new(Some(entity)),
            factory,
            cache,
            registry,
            last_string: RefCell::new(String::new()),
            glb_exporter,
        })
    }
}

impl IModelHandleImpl for ModelHandle {
    fn text_dump(&self) -> &str {
        *self.last_string.borrow_mut() = self.text_dump.clone();
        // SAFETY: see ConfigService::get_asset_path — single-threaded
        // script/UI path; codegen copies the &str into a CString immediately.
        unsafe { (*self.last_string.as_ptr()).as_str() }
    }

    fn open_preview(&self) -> ComRc<IPreviewSession> {
        let entity = self
            .entity
            .borrow_mut()
            .take()
            .expect("model handle preview can only be opened once");
        let scene = CoreScene::create();

        entity.load();
        scene.add_entity(entity);

        // Allocate the offscreen target. 512x512 is a reasonable initial
        // size for an editor tab; the script can call `target.resize(w,h)`
        // once it knows the imgui child window dimensions.
        let target_box = self.factory.create_render_target(512, 512);
        let (target_com, target_shared) =
            ScriptedRenderTarget::create(target_box, self.cache.clone());

        let orbit = OrbitState::new(Vec3::new(0., 0., 0.), 0., 0.3, 200.);
        let state = Rc::new(PreviewState {
            scene,
            target: target_shared,
            orbit: RefCell::new(orbit),
            closed: std::cell::Cell::new(false),
        });
        state.apply_camera();
        self.registry.register(&state);

        PreviewSession::create(state, target_com)
    }

    fn export_glb(&self, output_path: &str) -> bool {
        let Some(exporter) = self.glb_exporter.as_ref() else {
            log::warn!("export_glb called on a model without an exporter");
            return false;
        };
        let bytes = match exporter() {
            Ok(b) => b,
            Err(e) => {
                log::warn!("glb export failed: {:#}", e);
                return false;
            }
        };
        match std::fs::write(output_path, &bytes) {
            Ok(()) => true,
            Err(e) => {
                log::warn!("writing {} failed: {}", output_path, e);
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UI layout (CEGUI <GUILayout>)
// ---------------------------------------------------------------------------

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
    /// a `DrawCmd` without at least the normal-state image resolved.
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
                let candidates: [&Option<shared::loaders::cegui::layout::ImageRef>; 5] = [
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
        // SAFETY: see ConfigService::get_asset_path — single-threaded
        // script/UI path; codegen copies the &str into a CString immediately.
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
        // SAFETY: see ConfigService::get_asset_path.
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
        // SAFETY: see ConfigService::get_asset_path.
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
        // SAFETY: see ConfigService::get_asset_path.
        unsafe { (*self.type_scratch.as_ptr()).as_str() }
    }
}

/// Recursive walker that turns the layout tree into a flat draw list.
///
/// Each window resolves its `UnifiedAreaRect` against its parent's
/// world-space pixel rect, emits a draw cmd if it has an `Image`
/// property whose `set:` reference is resolved, and then recurses
/// into children. `AlwaysOnTop` children of the same parent are
/// emitted *after* their non-on-top siblings (matching CEGUI's
/// z-order semantics).
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

    // The window's own area is in parent-relative coordinates: scale
    // multiplies parent extent, offset is absolute pixels.
    let (rel_x, rel_y, rw, rh) = window.area.resolve(pw, ph);
    let ax = px + rel_x;
    let ay = py + rel_y;
    let aw = if rw > 0.0 { rw } else { pw };
    let ah = if rh > 0.0 { rh } else { ph };

    // Pick the primary (normal-state) image, preferring an explicit
    // `Image` property and falling back to `olNormalImage` (the
    // pattern PAL4 buttons use).
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
        // Pure text widget (no backing image): still emit a draw cmd
        // so the renderer can lay down the label inside the widget's
        // rect. `base.texture_com_id == -1` tells the renderer to
        // skip the imgui::Image call and only emit the text.
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
    refr: &shared::loaders::cegui::layout::ImageRef,
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

/// Recognised interactive widget types — used to decide whether
/// hover/press hit-testing applies to a draw cmd even when its
/// `ol*Image` alt states are missing. List is conservative: only
/// types that the PAL4 layouts actually use today.
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
    (
        rect.x as f32 / tw,
        rect.y as f32 / th,
        (rect.x + rect.width) as f32 / tw,
        (rect.y + rect.height) as f32 / th,
    )
}

/// Compute the imageset directory paired with a layout file's path.
/// PAL4's `ui.cpk` mounts at `gamedata/ui/`, so layouts live at
/// `gamedata/ui/layouts/X.xml` and their `.imageset` siblings at
/// `gamedata/ui/imagesets/X.imageset`. The general rule is "replace
/// the layout's `layouts` segment with `imagesets`" — implemented as
/// trimming the basename plus the trailing `/layouts` segment, then
/// appending `/imagesets`.
///
/// Returns a forward-slash vfs string. `PathBuf::join` on Windows
/// interleaves backslashes, which MiniFs treats as part of the path
/// component name; we must keep `/` separators throughout.
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
        // 1) Direct filename match.
        let lower = set_name.to_lowercase();
        let candidates = [
            format!("{}.imageset", set_name),
            format!("{}_0.imageset", set_name),
        ];
        for cand in &candidates {
            let p = self.join(cand);
            match vfs.read_to_end(&p) {
                Ok(b) => {
                    log::debug!(
                        "ImagesetIndex::load({}): filename candidate {} hit ({} bytes)",
                        set_name,
                        p,
                        b.len()
                    );
                    if let Ok(imageset) = cegui_imageset::parse_imageset_bytes(&b) {
                        if imageset.name.eq_ignore_ascii_case(set_name) {
                            let png = cegui_imageset::read_atlas_png(vfs, &p, &imageset)?;
                            return Ok((imageset, png));
                        } else {
                            log::debug!(
                                "ImagesetIndex::load({}): candidate {} parsed but Name='{}' mismatches",
                                set_name,
                                p,
                                imageset.name
                            );
                        }
                    }
                }
                Err(e) => {
                    log::debug!(
                        "ImagesetIndex::load({}): filename candidate {} read failed: {}",
                        set_name,
                        p,
                        e
                    );
                }
            }
        }
        // 2) Scan dir on demand.
        if !self.scanned {
            self.scan(vfs);
            self.scanned = true;
        }
        if let Some(p) = self.name_to_path.get(&lower) {
            log::debug!("ImagesetIndex::load({}): dir-scan hit -> {}", set_name, p);
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
        log::info!("ImagesetIndex::scan({})", self.dir);
        let entries = match vfs.entries(&self.dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!(
                    "ImagesetIndex::scan({}): vfs.entries failed: {}",
                    self.dir,
                    e
                );
                return;
            }
        };
        let mut hits = 0usize;
        let mut total = 0usize;
        for entry in entries.flatten() {
            total += 1;
            if !matches!(entry.kind, EntryKind::File) {
                continue;
            }
            // MiniFs's `LocalFs` returns names that include the full
            // path from the store root (e.g. `gamedata/ui2/ui/
            // imagesets/foo.imageset`); other stores return just the
            // basename. Take the file_name() either way.
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
            hits += 1;
        }
        log::info!(
            "ImagesetIndex::scan({}): {} entries, {} imagesets indexed",
            self.dir,
            total,
            hits
        );
    }
}
