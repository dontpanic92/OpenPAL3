use std::cell::Cell;
use std::rc::Rc;

use imgui::TextureId;
use radiance::rendering::{
    ComponentFactory, MaterialDef, RenderObjectHandle, RenderingComponent, Texture, TextureDef,
    VertexBuffer, VideoPlayer,
};
use radiance_scripting::services::{ImguiTextureCache, Texture as ScriptTexture};

struct DummyTexture;

impl Texture for DummyTexture {
    fn width(&self) -> u32 {
        1
    }
    fn height(&self) -> u32 {
        1
    }
}

struct MockFactory {
    uploads: Cell<usize>,
}

impl ComponentFactory for MockFactory {
    fn create_texture(&self, _texture_def: &TextureDef) -> Box<dyn Texture> {
        Box::new(DummyTexture)
    }

    fn create_imgui_texture(
        &self,
        _buffer: &[u8],
        _row_length: u32,
        _width: u32,
        _height: u32,
        _texture_id: Option<TextureId>,
    ) -> (Box<dyn Texture>, TextureId) {
        let upload = self.uploads.get() + 1;
        self.uploads.set(upload);
        (Box::new(DummyTexture), TextureId::new(upload))
    }

    fn remove_imgui_texture(&self, _texture_id: Option<TextureId>) {}

    fn create_render_object(
        &self,
        _vertices: VertexBuffer,
        _indices: Vec<u32>,
        _material_def: &MaterialDef,
        _host_dynamic: bool,
    ) -> RenderObjectHandle {
        panic!("not used by texture cache smoke test")
    }

    fn create_rendering_component(&self, _objects: Vec<RenderObjectHandle>) -> RenderingComponent {
        RenderingComponent::new()
    }

    fn create_video_player(&self) -> Box<VideoPlayer> {
        Box::new(VideoPlayer::new())
    }

    fn create_render_target(
        &self,
        _width: u32,
        _height: u32,
    ) -> Box<dyn radiance::rendering::RenderTarget> {
        panic!("not used by texture cache smoke test")
    }
}

#[allow(dead_code)]
fn _check_signatures(c: &mut ImguiTextureCache, com_id: i64) {
    let _: Option<TextureId> = c.resolve(com_id);
}

#[test]
fn resolve_is_idempotent_after_upload() {
    let factory = Rc::new(MockFactory {
        uploads: Cell::new(0),
    });
    let mut cache = ImguiTextureCache::new(factory.clone());
    let texture = ScriptTexture::create(1, 1, vec![255, 0, 0, 255], 0);

    let first = cache.upload(42, texture.clone()).expect("first upload");
    let second = cache.upload(42, texture).expect("cached upload");

    assert_eq!(first.id(), second.id());
    assert_eq!(Some(first.id()), cache.resolve(42).map(|id| id.id()));
    assert_eq!(factory.uploads.get(), 1);
}

// --- Frame-gated deletion queue --------------------------------------
// DELETION_GRACE_FRAMES is 3 in the cache; these tests assert a
// forgotten texture survives exactly that many `advance_frame` ticks
// before its cache entry is evicted, so a mid-frame drop never frees a
// texture an in-flight frame still references.

#[test]
fn forgotten_texture_survives_grace_frames_then_evicts() {
    let factory = Rc::new(MockFactory {
        uploads: Cell::new(0),
    });
    let mut cache = ImguiTextureCache::new(factory.clone());
    let texture = ScriptTexture::create(1, 1, vec![255, 0, 0, 255], 0);
    cache.upload(7, texture).expect("upload");

    // Simulate a handle Drop pushing the com_id onto the sink mid-frame.
    cache.pending_forgets_sink().borrow_mut().push(7);

    // Same-frame resolve still sees it (drains sink -> deletion queue,
    // stamped GRACE frames out; nothing matured yet).
    assert!(cache.resolve(7).is_some(), "alive same frame as forget");

    // GRACE = 3: survives ticks 1 and 2, evicted on tick 3.
    cache.advance_frame();
    assert!(cache.resolve(7).is_some(), "alive after 1 frame");
    cache.advance_frame();
    assert!(cache.resolve(7).is_some(), "alive after 2 frames");
    cache.advance_frame();
    assert!(cache.resolve(7).is_none(), "evicted after 3 frames");
}

#[test]
fn advance_frame_without_forgets_is_noop() {
    let factory = Rc::new(MockFactory {
        uploads: Cell::new(0),
    });
    let mut cache = ImguiTextureCache::new(factory.clone());
    let texture = ScriptTexture::create(1, 1, vec![255, 0, 0, 255], 0);
    cache.upload(3, texture).expect("upload");

    for _ in 0..10 {
        cache.advance_frame();
    }
    assert!(
        cache.resolve(3).is_some(),
        "a live texture is never evicted without a forget"
    );
}

#[test]
fn forget_queued_just_before_tick_still_honors_full_grace() {
    let factory = Rc::new(MockFactory {
        uploads: Cell::new(0),
    });
    let mut cache = ImguiTextureCache::new(factory.clone());
    let texture = ScriptTexture::create(1, 1, vec![255, 0, 0, 255], 0);
    cache.upload(9, texture).expect("upload");

    // Advance a few frames first so the counter isn't zero, then forget.
    cache.advance_frame();
    cache.advance_frame();
    cache.pending_forgets_sink().borrow_mut().push(9);
    // Observe the forget now so its grace is stamped from the current
    // frame (grace is measured from when the forget is processed).
    assert!(cache.resolve(9).is_some(), "alive same frame as forget");

    cache.advance_frame(); // grace 1
    assert!(cache.resolve(9).is_some());
    cache.advance_frame(); // grace 2
    assert!(cache.resolve(9).is_some());
    cache.advance_frame(); // grace 3 -> evicted
    assert!(cache.resolve(9).is_none());
}
