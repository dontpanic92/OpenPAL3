use imgui::TextureId;

use super::{
    texture::TextureDef, MaterialDef, RenderObject, RenderTarget, RenderingComponent, Texture,
    VertexBuffer, VideoPlayer,
};

pub trait ComponentFactory {
    fn create_texture(&self, texture_def: &TextureDef) -> Box<dyn Texture>;

    fn create_imgui_texture(
        &self,
        buffer: &[u8],
        row_length: u32,
        width: u32,
        height: u32,
        texture_id: Option<TextureId>,
    ) -> (Box<dyn Texture>, TextureId);

    fn remove_imgui_texture(&self, texture_id: Option<TextureId>);

    fn create_render_object(
        &self,
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material_def: &MaterialDef,
        host_dynamic: bool,
    ) -> Box<dyn RenderObject>;

    fn create_rendering_component(&self, objects: Vec<Box<dyn RenderObject>>)
        -> RenderingComponent;

    fn create_video_player(&self) -> Box<VideoPlayer>;

    /// Allocate a fresh offscreen color target sized `width` x `height`.
    /// The returned target is owned by the caller; the engine renders into
    /// it via [`RenderingEngine::render_scene_to_target`](super::RenderingEngine::render_scene_to_target).
    ///
    /// Backends that don't yet support offscreen rendering (vitagl) panic
    /// with a clear message — see the per-backend impl for status.
    fn create_render_target(&self, width: u32, height: u32) -> Box<dyn RenderTarget>;
}
