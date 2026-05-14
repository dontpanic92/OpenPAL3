//! Foreign-handle COM objects exposed to p7 by `IPreviewerHub`.
//!
//! Each handle wraps a chunk of host-owned state (decoded image pixels, an
//! audio source, a video player, a model entity + scene) and exposes a small
//! interface that scripts can call without ever touching raw Rust state.

use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;
use radiance::audio::{AudioMemorySource, AudioSourceState};
use radiance::comdef::{IEntity, IScene, ISceneManager};
use radiance::math::Vec3;
use radiance::rendering::VideoPlayer;
use radiance::scene::CoreScene;
use radiance::video::VideoStreamState;
use radiance_scripting::services::texture_cache::next_handle_com_id;
use radiance_scripting::services::ImguiTextureCache;

use crate::comdef::editor_services::{
    IAudioHandle, IAudioHandleImpl, IImageHandle, IImageHandleImpl, IModelHandle,
    IModelHandleImpl, IPreviewSession, IPreviewSessionImpl, IVideoHandle, IVideoHandleImpl,
};

// ---------------------------------------------------------------------------
// Image
// ---------------------------------------------------------------------------

pub struct ImageHandle {
    width: u32,
    height: u32,
    com_id: i64,
    cache: Rc<RefCell<ImguiTextureCache>>,
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
        cache
            .borrow_mut()
            .upload_pixels(com_id, pixels, width, height)?;
        Some(ComRc::from_object(Self {
            width,
            height,
            com_id,
            cache,
        }))
    }
}

impl Drop for ImageHandle {
    fn drop(&mut self) {
        self.cache.borrow_mut().forget(self.com_id);
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
        ComRc::from_object(Self {
            player: RefCell::new(player),
            width,
            height,
            com_id,
            cache,
            last_texture: RefCell::new(None),
        })
    }
}

impl Drop for VideoHandle {
    fn drop(&mut self) {
        self.cache.borrow_mut().forget(self.com_id);
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
    scene_manager: ComRc<ISceneManager>,
    scene: ComRc<IScene>,
    closed: std::cell::Cell<bool>,
}

ComObject_PreviewSession!(super::PreviewSession);

impl PreviewSession {
    pub fn create(
        scene_manager: ComRc<ISceneManager>,
        scene: ComRc<IScene>,
    ) -> ComRc<IPreviewSession> {
        ComRc::from_object(Self {
            scene_manager,
            scene,
            closed: std::cell::Cell::new(false),
        })
    }

    fn pop(&self) {
        if self.closed.replace(true) {
            return;
        }
        if let Some(top) = self.scene_manager.scene() {
            if std::ptr::eq(top.ptr_value(), self.scene.ptr_value()) {
                self.scene_manager.pop_scene();
            } else {
                log::warn!(
                    "preview session close skipped: scene_manager top is not our preview scene"
                );
            }
        }
    }
}

impl Drop for PreviewSession {
    fn drop(&mut self) {
        self.pop();
    }
}

impl IPreviewSessionImpl for PreviewSession {
    fn close(&self) {
        self.pop();
    }
}

pub struct ModelHandle {
    text_dump: String,
    entity: RefCell<Option<ComRc<IEntity>>>,
    scene_manager: ComRc<ISceneManager>,
    last_string: RefCell<String>,
}

ComObject_ModelHandle!(super::ModelHandle);

impl ModelHandle {
    pub fn create(
        scene_manager: ComRc<ISceneManager>,
        text_dump: String,
        entity: ComRc<IEntity>,
    ) -> ComRc<IModelHandle> {
        ComRc::from_object(Self {
            text_dump,
            entity: RefCell::new(Some(entity)),
            scene_manager,
            last_string: RefCell::new(String::new()),
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
        self.scene_manager.pop_scene();
        self.scene_manager.push_scene(scene.clone());

        entity.load();
        scene.add_entity(entity);
        scene
            .camera()
            .borrow_mut()
            .transform_mut()
            .set_position(&Vec3::new(0., 200., 200.))
            .look_at(&Vec3::new(0., 0., 0.));

        PreviewSession::create(self.scene_manager.clone(), scene)
    }
}
