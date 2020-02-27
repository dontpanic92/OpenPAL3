use crate::rendering::render_object::TEXTURE_MISSING_TEXTURE_FILE;
use image::RgbaImage;
use std::path::PathBuf;

pub struct Texture {
    path: PathBuf,
    image: RgbaImage,
}

impl Texture {
    pub fn new(path: &PathBuf) -> Self {
        let image = match image::open(path) {
            Ok(img) => img,
            Err(_) => image::load_from_memory(&TEXTURE_MISSING_TEXTURE_FILE).unwrap(),
        }
        .to_rgba();

        Self {
            path: path.clone(),
            image,
        }
    }

    pub fn data(&self) -> &[u8] {
        self.image.as_ref()
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }
}
