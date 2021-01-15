use super::{
    texture::TextureDef, Material, MaterialDef, RenderObject, RenderingComponent, Shader,
    ShaderDef, Texture, VertexBuffer,
};

pub trait ComponentFactory {
    fn create_texture(&self, texture_def: &TextureDef) -> Box<dyn Texture>;
    fn create_shader(&self, shader_def: &ShaderDef) -> Box<dyn Shader>;
    fn create_material(&self, material_def: &MaterialDef) -> Box<dyn Material>;
    fn create_render_object(
        &self,
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material_def: &MaterialDef,
        host_dynamic: bool,
    ) -> Box<dyn RenderObject>;
    fn create_rendering_component(&self, objects: Vec<Box<dyn RenderObject>>)
        -> RenderingComponent;
}
