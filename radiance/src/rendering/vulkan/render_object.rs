use super::buffer::{Buffer, BufferType};
use super::material::VulkanMaterial;
use super::uniform_buffers::DynamicUniformBufferManager;
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::vulkan::descriptor_managers::DescriptorManager;
use crate::rendering::RenderObject;
use ash::vk;
use ash::Device;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Weak;

pub struct VulkanRenderObject {
    dub_manager: Weak<DynamicUniformBufferManager>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    material: VulkanMaterial,
    per_object_descriptor_sets: vk::DescriptorSet,
    dub_index: usize,
}

impl VulkanRenderObject {
    pub fn new(
        object: &RenderObject,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
        dub_manager: &Arc<DynamicUniformBufferManager>,
        descriptor_manager: &DescriptorManager,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_buffer = if object.is_host_dynamic() {
            Buffer::new_dynamic_buffer_with_data(
                allocator,
                BufferType::Vertex,
                object.vertices().data(),
            )?
        } else {
            Buffer::new_device_buffer_with_data(
                allocator,
                BufferType::Vertex,
                object.vertices().data(),
                command_runner,
            )?
        };

        let index_buffer = Buffer::new_device_buffer_with_data(
            allocator,
            BufferType::Index,
            object.indices(),
            command_runner,
        )?;

        let material = VulkanMaterial::new(object.material(), device, allocator, command_runner)?;
        let per_object_descriptor_sets =
            descriptor_manager.allocate_per_object_descriptor_set(&material)?;
        let dub_index = dub_manager.allocate_buffer();

        Ok(Self {
            dub_manager: Arc::downgrade(dub_manager),
            vertex_buffer,
            index_buffer,
            material,
            per_object_descriptor_sets,
            dub_index,
        })
    }

    pub fn update(&mut self, object: &mut RenderObject) {
        let _ = self
            .vertex_buffer
            .copy_memory_from(object.vertices().data());
        object.is_dirty = false;
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }

    pub fn dub_index(&self) -> usize {
        self.dub_index
    }

    pub fn material(&self) -> &VulkanMaterial {
        &self.material
    }

    pub fn vk_descriptor_set(&self) -> vk::DescriptorSet {
        self.per_object_descriptor_sets
    }
}

impl Drop for VulkanRenderObject {
    fn drop(&mut self) {
        let dub_manager = self.dub_manager.upgrade().unwrap();
        dub_manager.deallocate_buffer(self.dub_index);
    }
}
