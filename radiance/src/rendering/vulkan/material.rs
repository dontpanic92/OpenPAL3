use super::{
    image::Image, image_view::ImageView, sampler::Sampler, shader::VulkanShader,
    VulkanRenderingEngine,
};
use crate::rendering::Material;
use ash::vk;
use std::error::Error;

pub struct VulkanMaterial {
    shader: VulkanShader,
    texture: Image,
    image_view: ImageView,
    sampler: Sampler,
}

impl VulkanMaterial {
    pub fn new(
        material: &dyn Material,
        engine: &mut VulkanRenderingEngine,
    ) -> Result<Self, Box<dyn Error>> {
        let device = engine.device();
        let command_runner = engine.adhoc_command_runner();
        let shader = VulkanShader::new(device, material.shader())?;
        let texture = material.textures().get(0).unwrap();
        let buffer = engine.create_staging_buffer_with_data(texture.data())?;
        let format = vk::Format::R8G8B8A8_UNORM;
        let mut texture = engine.create_image(texture.width(), texture.height())?;
        texture.transit_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &command_runner,
        )?;
        texture.copy_from(&buffer, &command_runner)?;
        texture.transit_layout(
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            &command_runner,
        )?;

        let image_view =
            ImageView::new_color_image_view(&engine.device(), texture.vk_image(), format)?;
        let sampler = Sampler::new(&engine.device())?;

        Ok(Self {
            shader,
            texture,
            image_view,
            sampler,
        })
    }

    pub fn shader(&self) -> &VulkanShader {
        &self.shader
    }

    pub fn texture(&self) -> &Image {
        &self.texture
    }

    pub fn image_view(&self) -> &ImageView {
        &self.image_view
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }
}
