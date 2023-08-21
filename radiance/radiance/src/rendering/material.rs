use image::ImageFormat;

use crate::rendering::texture::TextureStore;

use super::{texture::TextureDef, ShaderProgram};
use std::{io::Read, sync::Arc};

pub trait Material: downcast_rs::Downcast + std::fmt::Debug {}

downcast_rs::impl_downcast!(Material);

#[derive(Clone)]
pub struct MaterialDef {
    name: String,
    shader: ShaderProgram,
    textures: Vec<Arc<TextureDef>>,
    use_alpha: bool,
}

impl MaterialDef {
    pub fn new(
        name: String,
        shader: ShaderProgram,
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

    pub fn shader(&self) -> ShaderProgram {
        self.shader
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
                image::load_from_memory(&buf)
                    .or_else(|_| image::load_from_memory_with_format(&buf, ImageFormat::Tga))
                    .and_then(|img| Ok(img.to_rgba8()))
                    .ok()
            } else {
                None
            }
        });

        Self::create_internal(texture, use_alpha)
    }

    pub fn create2(texture_name: &str, data: Option<Vec<u8>>, use_alpha: bool) -> MaterialDef {
        let texture = TextureStore::get_or_update(texture_name, || {
            if let Some(data) = data {
                image::load_from_memory(&data)
                    .or_else(|_| image::load_from_memory_with_format(&data, ImageFormat::Tga))
                    .and_then(|img| Ok(img.to_rgba8()))
                    .ok()
            } else {
                None
            }
        });

        Self::create_internal(texture, use_alpha)
    }

    fn create_internal(texture_def: Arc<TextureDef>, use_alpha: bool) -> MaterialDef {
        MaterialDef::new(
            "simple_material".to_string(),
            ShaderProgram::TexturedNoLight,
            vec![texture_def],
            use_alpha,
        )
    }
}

pub struct LightMapMaterialDef;
impl LightMapMaterialDef {
    pub fn create<R: Read>(
        textures: Vec<&str>,
        get_reader: impl Fn(&str) -> Option<R>,
        use_alpha: bool,
    ) -> MaterialDef {
        let textures: Vec<Arc<TextureDef>> = textures
            .into_iter()
            .map(|name| {
                TextureStore::get_or_update(name, || {
                    let mut buf = Vec::new();
                    let b = match get_reader(name) {
                        None => radiance_assets::TEXTURE_WHITE_TEXTURE_FILE,
                        Some(mut reader) => {
                            reader.read_to_end(&mut buf).unwrap();
                            &buf
                        }
                    };

                    image::load_from_memory(b)
                        .or_else(|err| {
                            log::error!("Cannot load texture: {}", &err);
                            Err(err)
                        })
                        .ok()
                        .and_then(|img| Some(img.to_rgba8()))
                })
            })
            .collect();

        MaterialDef::new(
            "lightmap_material".to_string(),
            ShaderProgram::TexturedLightmap,
            textures,
            use_alpha,
        )
    }
}
