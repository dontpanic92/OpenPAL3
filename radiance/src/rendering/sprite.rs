use image::{ImageFormat, RgbaImage};

use super::{ComponentFactory, Texture};

pub struct Sprite {
    image: image::RgbaImage,
    texture: Box<dyn Texture>,
    texture_id: imgui::TextureId,
}

impl Sprite {
    pub fn load_from_image(image: RgbaImage, factory: &dyn ComponentFactory) -> Self {
        let (texture, texture_id) =
            factory.create_imgui_texture(image.as_raw(), 0, image.width(), image.height(), None);

        Self {
            image,
            texture,
            texture_id,
        }
    }

    pub fn load_from_buffer(
        buffer: &[u8],
        format: ImageFormat,
        factory: &dyn ComponentFactory,
    ) -> Self {
        let image = image::load_from_memory_with_format(buffer, format)
            .unwrap()
            .to_rgba8();
        Self::load_from_image(image, factory)
    }

    pub fn imgui_texture_id(&self) -> imgui::TextureId {
        self.texture_id
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }
}
