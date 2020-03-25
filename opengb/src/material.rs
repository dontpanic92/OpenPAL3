use radiance::rendering::{Material, Shader, Texture, VertexComponents};
use std::path::PathBuf;

static LIGHTMAP_TEXTURE_VERT: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.vert.spv"));
static LIGHTMAP_TEXTURE_FRAG: &'static [u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/lightmap_texture.frag.spv"));
pub static WHITE_TEXTURE_FILE: &'static [u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embed/textures/white.png"
));

pub struct LightMapShader {}

impl Shader for LightMapShader {
    fn name(&self) -> &str {
        "lightmap_texture"
    }

    fn vertex_components(&self) -> VertexComponents {
        VertexComponents::POSITION | VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2
    }

    fn vert_src(&self) -> &[u8] {
        LIGHTMAP_TEXTURE_VERT
    }

    fn frag_src(&self) -> &[u8] {
        LIGHTMAP_TEXTURE_FRAG
    }
}

pub struct LightMapMaterial {
    textures: Vec<Texture>,
    shader: LightMapShader,
}

impl LightMapMaterial {
    pub fn new(texture_paths: &[PathBuf]) -> Self {
        let textures: Vec<Texture> = texture_paths
            .iter()
            .map(|p| {
                if p.file_stem() == None {
                    Texture::new_with_iamge(
                        image::load_from_memory(&WHITE_TEXTURE_FILE)
                            .unwrap()
                            .to_rgba(),
                    )
                } else {
                    Texture::new(p)
                }
            })
            .collect();
        LightMapMaterial {
            textures,
            shader: LightMapShader {},
        }
    }
}

impl Material for LightMapMaterial {
    fn name(&self) -> &str {
        "lightmap_material"
    }

    fn shader(&self) -> &dyn Shader {
        &self.shader
    }

    fn textures(&self) -> &[Texture] {
        &self.textures
    }
}
