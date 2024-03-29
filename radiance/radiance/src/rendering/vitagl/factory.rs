use std::{cell::RefCell, collections::HashMap, rc::Rc};

use imgui::TextureId;

use crate::rendering::{
    ComponentFactory, Material, MaterialDef, RenderObject, RenderingComponent, Shader,
    ShaderProgram, Texture, TextureDef, VertexBuffer, VideoPlayer,
};

use super::{
    material::VitaGLMaterial, render_object::VitaGLRenderObject, shader::VitaGLShader,
    texture::VitaGLTexture,
};

pub struct VitaGLComponentFactory {
    shaders: RefCell<HashMap<ShaderProgram, Rc<VitaGLShader>>>,
}

impl ComponentFactory for VitaGLComponentFactory {
    fn create_texture(&self, texture_def: &TextureDef) -> Box<dyn Texture> {
        let rgba_image = texture_def
            .image()
            .unwrap_or_else(|| &TEXTURE_MISSING_IMAGE);
        Box::new(VitaGLTexture::new(
            rgba_image.width(),
            rgba_image.height(),
            &rgba_image,
        ))
    }

    fn create_imgui_texture(
        &self,
        buffer: &[u8],
        row_length: u32,
        width: u32,
        height: u32,
        texture_id: Option<TextureId>,
    ) -> (Box<dyn Texture>, TextureId) {
        let texture = VitaGLTexture::new(width, height, buffer);
        let texture_id = texture.texture_id();
        (Box::new(texture), TextureId::new(texture_id as usize))
    }

    fn remove_imgui_texture(&self, texture_id: Option<TextureId>) {}

    fn create_material(&self, material_def: &MaterialDef) -> Box<dyn Material> {
        Box::new(VitaGLMaterial::new(
            material_def,
            self.create_shader(material_def.shader()),
        ))
    }

    fn create_render_object(
        &self,
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material_def: &MaterialDef,
        host_dynamic: bool,
    ) -> Box<dyn RenderObject> {
        let material = self.create_material(material_def);
        let x =
            Box::new(VitaGLRenderObject::new(vertices, indices, material, host_dynamic).unwrap());
        x
    }

    fn create_rendering_component(
        &self,
        objects: Vec<Box<dyn RenderObject>>,
    ) -> RenderingComponent {
        let mut component = RenderingComponent::new();
        for o in objects {
            component.push_render_object(o);
        }

        component
    }

    fn create_video_player(&self) -> Box<VideoPlayer> {
        Box::new(VideoPlayer::new())
    }
}

impl VitaGLComponentFactory {
    pub fn new() -> Self {
        Self {
            shaders: RefCell::new(HashMap::new()),
        }
    }

    fn create_shader(&self, shader: ShaderProgram) -> Rc<VitaGLShader> {
        self.shaders
            .borrow_mut()
            .entry(shader)
            .or_insert_with(|| Rc::new(VitaGLShader::new(shader).unwrap()))
            .clone()
    }
}

lazy_static::lazy_static! {
    static ref TEXTURE_MISSING_IMAGE: image::RgbaImage
         = image::load_from_memory(radiance_assets::TEXTURE_MISSING_TEXTURE_FILE).unwrap().to_rgba8();
}
