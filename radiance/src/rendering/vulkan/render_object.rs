use super::buffer::{Buffer, BufferType};
use super::descriptor_sets::DescriptorSets;
use super::image::Image;
use super::image_view::ImageView;
use super::sampler::Sampler;
use super::VulkanRenderingEngine;
use super::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::RenderObject;
use ash::vk;
use image::GenericImageView;
use std::error::Error;
use std::rc::{Rc, Weak};

pub struct VulkanRenderObject {
    vertex_staging_buffer: Buffer,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    texture: Image,
    image_view: ImageView,
    sampler: Sampler,
    per_object_descriptor_sets: DescriptorSets,
    command_runner: Weak<AdhocCommandRunner>,
}

impl VulkanRenderObject {
    pub fn new(
        object: &RenderObject,
        engine: &mut VulkanRenderingEngine,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_staging_buffer = 
            engine.create_staging_buffer_with_data(object.vertices())?;
        let vertex_buffer = 
            engine.create_device_buffer_with_data(BufferType::Vertex,  object.vertices())?;
        let index_buffer =
            engine.create_device_buffer_with_data(BufferType::Index, object.indices())?;

        let command_runner = engine.adhoc_command_runner();
        let texture = object.material().textures()[0];
        let buffer = engine.create_staging_buffer_with_data(texture.data())?;
        let format = vk::Format::R8G8B8A8_UNORM;
        let mut texture = engine.create_image(texture.width(), texture.height())?;
        texture.transit_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            command_runner.as_ref(),
        )?;
        texture.copy_from(&buffer, command_runner.as_ref())?;
        texture.transit_layout(
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            command_runner.as_ref(),
        )?;

        let image_view =
            ImageView::new_color_image_view(&engine.device(), texture.vk_image(), format)?;
        let sampler = Sampler::new(&engine.device())?;
        let per_object_descriptor_sets = engine
            .descriptor_manager_mut()
            .allocate_per_object_descriptor_set(&image_view, &sampler)?;

        Ok(Self {
            vertex_staging_buffer,
            vertex_buffer,
            index_buffer,
            texture,
            image_view,
            sampler,
            per_object_descriptor_sets,
            command_runner: Rc::downgrade(&command_runner),
        })
    }

    pub fn update(&mut self, object: &mut RenderObject) {
        if let Some(command_runner) = self.command_runner.upgrade() {
            let _ = self.vertex_staging_buffer.memory_mut().copy_from(object.vertices());
            let _ = self.vertex_buffer.copy_from(&self.vertex_staging_buffer, command_runner.as_ref());

            object.is_dirty = false;
        }
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
