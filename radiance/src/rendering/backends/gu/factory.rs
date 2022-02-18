use crate::rendering::ComponentFactory;

pub struct GuComponentFactory {}

impl GuComponentFactory {
    pub fn new() -> Self {
        Self {}
    }
}

impl ComponentFactory for GuComponentFactory {
    fn create_texture(
        &self,
        texture_def: &crate::rendering::TextureDef,
    ) -> alloc::boxed::Box<dyn crate::rendering::Texture> {
        todo!()
    }

    fn create_imgui_texture(
        &self,
        buffer: &[u8],
        row_length: u32,
        width: u32,
        height: u32,
        texture_id: Option<crate::rendering::ui::TextureId>,
    ) -> (
        alloc::boxed::Box<dyn crate::rendering::Texture>,
        crate::rendering::ui::TextureId,
    ) {
        todo!()
    }

    fn create_shader(
        &self,
        shader_def: &crate::rendering::ShaderDef,
    ) -> alloc::boxed::Box<dyn crate::rendering::Shader> {
        todo!()
    }

    fn create_material(
        &self,
        material_def: &crate::rendering::MaterialDef,
    ) -> alloc::boxed::Box<dyn crate::rendering::Material> {
        todo!()
    }

    fn create_render_object(
        &self,
        vertices: crate::rendering::VertexBuffer,
        indices: alloc::vec::Vec<u32>,
        material_def: &crate::rendering::MaterialDef,
        host_dynamic: bool,
    ) -> alloc::boxed::Box<dyn crate::rendering::RenderObject> {
        todo!()
    }

    fn create_rendering_component(
        &self,
        objects: alloc::vec::Vec<alloc::boxed::Box<dyn crate::rendering::RenderObject>>,
    ) -> crate::rendering::RenderingComponent {
        todo!()
    }

    fn create_video_player(&self) -> alloc::boxed::Box<crate::rendering::VideoPlayer> {
        todo!()
    }
}
