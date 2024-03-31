use super::adhoc_command_runner::AdhocCommandRunner;
use ash::prelude::VkResult;
use ash::vk;
use std::error::Error;
use std::mem::size_of;
use std::rc::Rc;
use vma::Alloc;

pub enum BufferType {
    Index = 0,
    Vertex = 1,
    Uniform = 2,
}

impl BufferType {
    pub fn to_buffer_usage(&self) -> vk::BufferUsageFlags {
        match self {
            BufferType::Index => vk::BufferUsageFlags::INDEX_BUFFER,
            BufferType::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
            BufferType::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
        }
    }
}

pub struct Buffer {
    allocator: Rc<vma::Allocator>,
    buffer: vk::Buffer,
    allocation: vma::Allocation,
    buffer_size: u64,
    element_size: usize,
    element_count: u32,
}

impl Buffer {
    pub fn new_staging_buffer_with_data<T>(
        allocator: &Rc<vma::Allocator>,
        data: &[T],
    ) -> Result<Self, Box<dyn Error>> {
        let mut staging_buffer = Buffer::new_buffer(
            allocator,
            size_of::<T>(),
            data.len(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            vma::MemoryUsage::AutoPreferHost,
        )?;

        staging_buffer.copy_memory_from(data);
        Ok(staging_buffer)
    }

    pub fn new_dynamic_buffer_with_data<T>(
        allocator: &Rc<vma::Allocator>,
        buffer_type: BufferType,
        data: &[T],
    ) -> Result<Self, Box<dyn Error>> {
        let mut dynamic_buffer = Buffer::new_buffer(
            allocator,
            size_of::<T>(),
            data.len(),
            buffer_type.to_buffer_usage(),
            vma::MemoryUsage::Auto,
        )?;

        dynamic_buffer.copy_memory_from(data);
        Ok(dynamic_buffer)
    }

    pub fn new_dynamic_buffer(
        allocator: &Rc<vma::Allocator>,
        buffer_type: BufferType,
        element_size: usize,
        element_count: usize,
    ) -> Result<Self, Box<dyn Error>> {
        Self::new_buffer(
            allocator,
            element_size,
            element_count,
            buffer_type.to_buffer_usage(),
            vma::MemoryUsage::Auto,
        )
    }

    pub fn new_device_buffer_with_data<T>(
        allocator: &Rc<vma::Allocator>,
        buffer_type: BufferType,
        data: &[T],
        command_runner: &AdhocCommandRunner,
    ) -> Result<Self, Box<dyn Error>> {
        let staging_buffer = Buffer::new_staging_buffer_with_data(allocator, data)?;

        let flags = match buffer_type {
            BufferType::Index => vk::BufferUsageFlags::INDEX_BUFFER,
            BufferType::Vertex => vk::BufferUsageFlags::VERTEX_BUFFER,
            BufferType::Uniform => vk::BufferUsageFlags::UNIFORM_BUFFER,
        };

        let mut buffer = Buffer::new_buffer(
            allocator,
            size_of::<T>(),
            data.len(),
            flags | vk::BufferUsageFlags::TRANSFER_DST,
            vma::MemoryUsage::AutoPreferDevice,
        )?;

        buffer.copy_from(&staging_buffer, command_runner)?;
        Ok(buffer)
    }

    pub fn vk_buffer(&self) -> vk::Buffer {
        self.buffer
    }

    pub fn element_count(&self) -> u32 {
        self.element_count
    }

    pub fn element_size(&self) -> usize {
        self.element_size
    }

    pub fn size(&self) -> u64 {
        self.buffer_size
    }

    fn new_buffer(
        allocator: &Rc<vma::Allocator>,
        element_size: usize,
        element_count: usize,
        buffer_usage: vk::BufferUsageFlags,
        memory_usage: vma::MemoryUsage,
    ) -> Result<Self, Box<dyn Error>> {
        let buffer_size = element_count as u64 * element_size as u64;
        let (buffer, allocation) = {
            let allcation_create_info = vma::AllocationCreateInfo {
                usage: memory_usage,
                flags: if memory_usage == vma::MemoryUsage::Auto
                    || memory_usage == vma::MemoryUsage::AutoPreferHost
                {
                    vma::AllocationCreateFlags::MAPPED
                        | vma::AllocationCreateFlags::HOST_ACCESS_RANDOM
                } else {
                    vma::AllocationCreateFlags::empty()
                },
                ..Default::default()
            };

            let buffer_create_info = vk::BufferCreateInfo::builder()
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .usage(buffer_usage)
                .size(buffer_size)
                .build();

            unsafe { allocator.create_buffer(&buffer_create_info, &allcation_create_info) }.unwrap()
        };

        Ok(Self {
            allocator: allocator.clone(),
            buffer,
            allocation,
            buffer_size,
            element_size,
            element_count: element_count as u32,
        })
    }

    pub fn copy_from(
        &mut self,
        src_buffer: &Buffer,
        command_runner: &AdhocCommandRunner,
    ) -> VkResult<()> {
        command_runner.run_commands_one_shot(|device, &command_buffer| {
            let copy_region = vk::BufferCopy::builder()
                .size(src_buffer.buffer_size)
                .build();
            device.cmd_copy_buffer(
                command_buffer,
                src_buffer.buffer,
                self.buffer,
                &[copy_region],
            );
        })
    }

    pub fn copy_memory_from<T>(&mut self, data: &[T]) {
        self.map_memory_do(|dst| {
            unsafe {
                std::ptr::copy(
                    data.as_ptr() as *const u8,
                    dst,
                    data.len() * std::mem::size_of::<T>(),
                )
            };
        });
    }

    pub fn map_memory_do<F: Fn(*mut u8)>(&mut self, action: F) {
        let dst = unsafe { self.allocator.map_memory(&mut self.allocation).unwrap() };

        if dst == std::ptr::null_mut() {
            panic!("Unable to map the dest memory");
        }

        action(dst);

        unsafe {
            self.allocator.unmap_memory(&mut self.allocation);
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.allocator
                .destroy_buffer(self.buffer, &mut self.allocation);
        }
    }
}
