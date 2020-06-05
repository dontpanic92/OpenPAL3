use radiance::rendering::{MaterialDef, ShaderDef, TextureDef, VertexComponents};
use std::path::PathBuf;

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
    pub fn create(texture_paths: &[PathBuf]) -> MaterialDef {
        let textures: Vec<TextureDef> = texture_paths
            .iter()
            .map(|p| {
                if p.file_stem() == None {
                    TextureDef::ImageTextureDef(
                        image::load_from_memory(&WHITE_TEXTURE_FILE)
                            .unwrap()
                            .to_rgba(),
                    )
                } else {
                    TextureDef::PathTextureDef(p.clone())
                }
            })
            .collect();

        MaterialDef::new("lightmap_material", LightMapShaderDef::create(), textures)
    }
}
