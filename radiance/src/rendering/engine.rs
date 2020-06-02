use super::{
    imgui::{ImguiContext, ImguiFrame},
    texture::TextureDef,
    Material, MaterialDef, RenderObject, Shader, ShaderDef, Texture, VertexBuffer,
};
use crate::scene::Scene;

pub trait RenderingEngine {
    fn create_texture(&self, texture_def: &TextureDef) -> Box<dyn Texture>;
    fn create_shader(&self, shader_def: &ShaderDef) -> Box<dyn Shader>;
    fn create_material(&self, material_def: &MaterialDef) -> Box<dyn Material>;
    fn create_render_object(
        &self,
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material: Box<dyn Material>,
        host_dynamic: bool,
    ) -> Box<dyn RenderObject>;
}

pub(crate) trait RenderingEngineInternal: RenderingEngine {
    fn render(&mut self, scene: &mut dyn Scene, ui_frame: ImguiFrame);
    fn scene_loaded(&mut self, scene: &mut dyn Scene);

    fn view_extent(&self) -> (u32, u32);
    fn gui_context_mut(&mut self) -> &mut ImguiContext;

    fn as_rendering_engine(&mut self) -> &mut dyn RenderingEngine;
}
