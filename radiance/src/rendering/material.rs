use super::{image::RgbaImage, texture::TextureDef, ShaderDef, SIMPLE_SHADER_DEF};
use crate::rendering::texture::TextureStore;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::{string::String, vec, vec::Vec};
use core2::io::Read;

pub trait Material: downcast_rs::Downcast + core::fmt::Debug {}

downcast_rs::impl_downcast!(Material);

pub struct MaterialDef {
    name: String,
    shader: ShaderDef,
    textures: Vec<Arc<TextureDef>>,
    use_alpha: bool,
}

impl MaterialDef {
    pub fn new(
        name: String,
        shader: ShaderDef,
        textures: Vec<Arc<TextureDef>>,
        use_alpha: bool,
    ) -> Self {
        Self {
            name,
            textures,
            shader,
            use_alpha,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shader(&self) -> &ShaderDef {
        &self.shader
    }

    pub fn textures(&self) -> &[Arc<TextureDef>] {
        &self.textures
    }

    pub fn use_alpha(&self) -> bool {
        self.use_alpha
    }
}

pub struct SimpleMaterialDef;
impl SimpleMaterialDef {
    pub fn create<R: Read>(
        texture_name: &str,
        get_reader: impl FnOnce(&str) -> Option<R>,
        use_alpha: bool,
    ) -> MaterialDef {
        let texture = TextureStore::get_or_update(texture_name, || {
            if let Some(mut r) = get_reader(texture_name) {
                let mut buf = Vec::new();
                r.read_to_end(&mut buf).unwrap();
                RgbaImage::load_from_memory(&buf, texture_name)
            } else {
                None
            }
        });

        Self::create_internal(texture, use_alpha)
    }

    fn create_internal(texture_def: Arc<TextureDef>, use_alpha: bool) -> MaterialDef {
        MaterialDef::new(
            "simple_material".to_string(),
            SIMPLE_SHADER_DEF.clone(),
            vec![texture_def],
            use_alpha,
        )
    }
}
