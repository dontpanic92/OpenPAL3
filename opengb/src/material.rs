use radiance::rendering::{MaterialDef, ShaderDef, TextureDef, TextureStore, VertexComponents, RgbaImage};
use std::{io::Read, sync::Arc};

static LIGHTMAP_TEXTURE_VERT: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.vert.spv"));
static LIGHTMAP_TEXTURE_FRAG: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.frag.spv"));
pub static WHITE_TEXTURE_FILE: &'static [u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embed/textures/white.png"
));

pub struct LightMapShaderDef;
impl LightMapShaderDef {
    pub fn create() -> ShaderDef {
        ShaderDef::new(
            "lightmap_texture",
            VertexComponents::POSITION | VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2,
            LIGHTMAP_TEXTURE_VERT,
            LIGHTMAP_TEXTURE_FRAG,
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
                        None => WHITE_TEXTURE_FILE,
                        Some(mut reader) => {
                            reader.read_to_end(&mut buf).unwrap();
                            &buf
                        }
                    };

                    RgbaImage::load_from_memory(b, name)
                        .or_else(|| {
                            log::error!("Cannot load texture");
                            None
                        })
                })
            })
            .collect();

        MaterialDef::new(
            "lightmap_material".to_string(),
            LightMapShaderDef::create(),
            textures,
            use_alpha,
        )
    }
}
