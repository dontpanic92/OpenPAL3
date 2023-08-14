use crate::rendering::Texture;

pub struct VitaGLTexture {}

impl Texture for VitaGLTexture {
    fn width(&self) -> u32 {
        0
    }

    fn height(&self) -> u32 {
        0
    }
}

impl VitaGLTexture {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self{})
    }
}
