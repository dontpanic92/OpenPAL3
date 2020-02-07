use super::buffer::{Buffer, BufferType};
use super::descriptor_sets::DescriptorSets;
use super::image::Image;
use super::image_view::ImageView;
use super::sampler::Sampler;
use super::VulkanRenderingEngine;
use crate::rendering::RenderObject;
use ash::vk;
use image::GenericImageView;
use std::error::Error;

pub struct VulkanRenderObject {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    texture: Image,
    image_view: ImageView,
    sampler: Sampler,
    per_object_descriptor_sets: DescriptorSets,
}

impl VulkanRenderObject {
    pub fn new(
        engine: &mut VulkanRenderingEngine,
        object: &RenderObject,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_buffer =
            engine.create_device_buffer_with_data(BufferType::Vertex, &object.vertices())?;
        let index_buffer =
            engine.create_device_buffer_with_data(BufferType::Index, &object.indices())?;

        let texture_image = image::open(object.texture_path()).unwrap();
        let data = texture_image.to_rgba();
        let buffer = engine.create_staging_buffer_with_data(data.as_ref())?;
        let mut texture = engine.create_image(texture_image.width(), texture_image.height())?;
        texture.transit_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            engine,
        )?;
        texture.copy_from(&buffer, engine)?;
        texture.transit_layout(
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            engine,
        )?;

        let image_view = ImageView::new(
            engine.device(),
            texture.vk_image(),
            vk::Format::R8G8B8A8_UNORM,
        )?;
        let sampler = Sampler::new(engine.device())?;
        let per_object_descriptor_sets = engine
            .descriptor_manager_mut()
            .allocate_per_object_descriptor_set(&image_view, &sampler)?;

        Ok(Self {
            vertex_buffer,
            index_buffer,
            texture,
            image_view,
            sampler,
            per_object_descriptor_sets,
        })
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }

    pub fn image_view(&self) -> &ImageView {
        &self.image_view
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    pub fn vk_descriptor_set(&self) -> vk::DescriptorSet {
        self.per_object_descriptor_sets.vk_descriptor_set()[0]
    }
}
