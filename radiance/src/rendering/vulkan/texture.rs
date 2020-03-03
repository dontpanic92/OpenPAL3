use super::{
    buffer::Buffer, image::Image, image_view::ImageView, sampler::Sampler, VulkanRenderingEngine,
};
use crate::rendering::texture::Texture;
use ash::vk;
use std::error::Error;

pub struct VulkanTexture {
    image: Image,
    image_view: ImageView,
    sampler: Sampler,
}

impl VulkanTexture {
    pub fn new(
        texture: &Texture,
        engine: &mut VulkanRenderingEngine,
    ) -> Result<Self, Box<dyn Error>> {
        let device = engine.device();
        let command_runner = engine.adhoc_command_runner();
        let buffer = Buffer::new_staging_buffer_with_data(engine.allocator(), texture.data())?;
        let format = vk::Format::R8G8B8A8_UNORM;
        let mut image =
            Image::new_color_image(engine.allocator(), texture.width(), texture.height())?;
        image.transit_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &command_runner,
        )?;
        image.copy_from(&buffer, &command_runner)?;
        image.transit_layout(
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            &command_runner,
        )?;

        let image_view = ImageView::new_color_image_view(device, image.vk_image(), format)?;
        let sampler = Sampler::new(device)?;

        Ok(Self {
            image,
            image_view,
            sampler,
        })
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn image_view(&self) -> &ImageView {
        &self.image_view
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }
}
