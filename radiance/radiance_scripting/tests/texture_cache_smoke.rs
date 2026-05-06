use std::cell::Cell;
use std::rc::Rc;

use imgui::TextureId;
use radiance::rendering::{
    ComponentFactory, Material, MaterialDef, RenderObject, RenderingComponent, Texture, TextureDef,
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

    fn create_material(&self, _material_def: &MaterialDef) -> Box<dyn Material> {
        panic!("not used by texture cache smoke test")
    }

    fn create_render_object(
        &self,
        _vertices: VertexBuffer,
        _indices: Vec<u32>,
        _material_def: &MaterialDef,
        _host_dynamic: bool,
    ) -> Box<dyn RenderObject> {
        panic!("not used by texture cache smoke test")
    }

    fn create_rendering_component(
        &self,
        _objects: Vec<Box<dyn RenderObject>>,
    ) -> RenderingComponent {
        RenderingComponent::new()
    }

    fn create_video_player(&self) -> Box<VideoPlayer> {
        Box::new(VideoPlayer::new())
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
