use crate::rendering::ui::TextureId;
use alloc::boxed::Box;

use super::{
    image::{ImageFormat, RgbaImage},
    ComponentFactory, Texture,
};

pub struct Sprite {
    image: RgbaImage,
    _texture: Box<dyn Texture>,
    texture_id: TextureId,
}

impl Sprite {
    pub fn load_from_image(image: RgbaImage, factory: &dyn ComponentFactory) -> Self {
        let (texture, texture_id) =
            factory.create_imgui_texture(image.as_raw(), 0, image.width(), image.height(), None);

        Self {
            image,
            _texture: texture,
            texture_id,
        }
    }

    pub fn load_from_buffer(
        buffer: &[u8],
        format: ImageFormat,
        factory: &dyn ComponentFactory,
    ) -> Self {
        let image = RgbaImage::load_from_memory_with_format(buffer, format).unwrap();
        Self::load_from_image(image, factory)
    }

    pub fn imgui_texture_id(&self) -> TextureId {
        self.texture_id
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }
}
