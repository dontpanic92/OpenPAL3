//! Foreign-handle COM objects exposed to p7 by `IPreviewerHub`.
//!
//! Each handle wraps a chunk of host-owned state (decoded image pixels, an
//! audio source, a video player, a model entity + scene) and exposes a small
//! interface that scripts can call without ever touching raw Rust state.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::audio::{AudioMemorySource, AudioSourceState};
use radiance::comdef::IEntity;
use radiance::math::Vec3;
use radiance::rendering::ComponentFactory;
use radiance::scene::CoreScene;

use radiance_scripting::comdef::services::IRenderTarget;
use radiance_scripting::services::texture_cache::next_handle_com_id;
use radiance_scripting::services::{ImguiTextureCache, ScriptedRenderTarget};

use crate::comdef::editor_services::{
    IAudioHandle, IAudioHandleImpl, IImageHandle, IImageHandleImpl, IModelHandle, IModelHandleImpl,
    IPreviewSession, IPreviewSessionImpl,
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
    /// frame (`ImguiUiFrameRenderer::render_frame`). See
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
//
// `VideoHandle` impl moved to `shared::loaders::video_handle` so the
// PAL3 menu intro-movie path can reuse the same code. The editor
// only re-exports the constructor below for `previewer_hub::open_video`.
pub use shared::loaders::video_handle::VideoHandle;

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
//
// The full impl lives in `shared::loaders::cegui::ui_layout_handle` next to
// the CEGUI parsers; both the PAL4 runtime start-menu director and the
// editor previewer mint instances through `UiLayoutHandle::try_create`.
pub use shared::loaders::cegui::ui_layout_handle::UiLayoutHandle;
