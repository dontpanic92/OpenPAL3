use radiance::rendering::{MaterialDef, ShaderDef, TextureDef, VertexComponents};
use std::io::Read;

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
    pub fn create<R: Read>(readers: &mut [Option<R>]) -> MaterialDef {
        let textures: Vec<TextureDef> = readers
            .iter_mut()
            .map(|r| {
                let mut buf = Vec::new();
                let b = match r {
                    None => WHITE_TEXTURE_FILE,
                    Some(reader) => {
                        reader.read_to_end(&mut buf).unwrap();
                        &buf
                    }
                };

                TextureDef::ImageTextureDef(Some(image::load_from_memory(b).unwrap().to_rgba8()))
            })
            .collect();

        MaterialDef::new("lightmap_material", LightMapShaderDef::create(), textures)
    }
}
