use super::buffer::{Buffer, BufferType};
use crate::math::Mat44;
use crate::rendering::vulkan::descriptor_managers::DynamicUniformBufferDescriptorManager;
use ash::vk;
use std::{rc::Rc, sync::Mutex};

#[derive(Clone)]
#[repr(C)]
pub struct PerInstanceUniformBuffer {
    model: Mat44,
}

impl PerInstanceUniformBuffer {
    pub fn new(model: &Mat44) -> Self {
        Self { model: *model }
    }
}

pub struct DynamicUniformBufferManager {
    descriptor_set: vk::DescriptorSet,
    alignment: u64,
    buffer: Buffer,
    usage: Mutex<Vec<bool>>,
}

impl DynamicUniformBufferManager {
    pub fn new(
        allocator: &Rc<vk_mem::Allocator>,
        descriptor_manager: &DynamicUniformBufferDescriptorManager,
        min_alignment: u64,
    ) -> Self {
        let (alignment, buffer) = Self::allocate_dynamic_buffer(allocator, min_alignment, 10240);
        let descriptor_set = descriptor_manager.allocate_descriptor_sets(&buffer);

        Self {
            descriptor_set,
            alignment,
            buffer,
            usage: Mutex::new(vec![false; 10240]),
        }
    }

    pub fn allocate_buffer(&self) -> usize {
        let mut usage = self.usage.lock().unwrap();
        for i in 0..usage.len() {
            if usage[i] == false {
                usage[i] = true;
                return i;
            }
        }

        panic!("No uniform buffer available");
    }

    pub fn descriptor_set(&self) -> vk::DescriptorSet {
        self.descriptor_set
    }

    pub fn update_do<F: Fn(&dyn Fn(usize, &Mat44))>(&self, action: F) {
        self.buffer.map_memory_do(|dst| {
            let updater = |id: usize, model: &Mat44| {
                let uniform_buffer: &mut PerInstanceUniformBuffer = unsafe {
                    &mut *(dst.offset(self.get_offset(id) as isize) as *mut _
                        as *mut PerInstanceUniformBuffer)
                };

                uniform_buffer.model = model.clone();
            };

            action(&updater);
        });
    }

    pub fn get_offset(&self, id: usize) -> u64 {
        self.alignment * id as u64
    }

    pub fn deallocate_buffer(&self, id: usize) {
        let mut usage = self.usage.lock().unwrap();
        usage[id] = false;
    }

    fn allocate_dynamic_buffer(
        allocator: &Rc<vk_mem::Allocator>,
        min_alignment: u64,
        buffer_count: u32,
    ) -> (u64, Buffer) {
        let alignment = (std::mem::size_of::<PerInstanceUniformBuffer>() as u64 + min_alignment)
            & !(min_alignment - 1);
        let buffer = Buffer::new_dynamic_buffer(
            allocator,
            BufferType::Uniform,
            alignment as usize,
            buffer_count as usize,
        )
        .unwrap();

        (alignment, buffer)
    }
}

#[repr(C)]
pub struct PerFrameUniformBuffer {
    view: Mat44,
    projection: Mat44,
}

impl PerFrameUniformBuffer {
    pub fn new(view: &Mat44, projection: &Mat44) -> Self {
        Self {
            view: *view,
            projection: *projection,
        }
    }
}
