use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::rc::{Rc, Weak};

pub struct AdhocCommandRunner {
    device: Weak<Device>,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
}

impl AdhocCommandRunner {
    pub fn new(device: &Rc<Device>, command_pool: vk::CommandPool, queue: vk::Queue) -> Self {
        Self {
            device: Rc::downgrade(device),
            command_pool,
            queue,
        }
    }

    pub fn run_commands_one_shot<F: FnOnce(&Rc<Device>, &vk::CommandBuffer)>(
        &self,
        record_command: F,
    ) -> VkResult<()> {
        let device = self.device.upgrade().unwrap();
        let command_buffers = {
            let allocation_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(self.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1)
                .build();
            unsafe { device.allocate_command_buffers(&allocation_info)? }
        };

        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe { device.begin_command_buffer(command_buffers[0], &begin_info)? };

        record_command(&device, &command_buffers[0]);

        unsafe {
            device.end_command_buffer(command_buffers[0])?;

            let submit_info = vk::SubmitInfo::builder()
                .command_buffers(&command_buffers)
                .build();
            device.queue_submit(self.queue, &[submit_info], vk::Fence::default())?;
            device.queue_wait_idle(self.queue)?;
            device.free_command_buffers(self.command_pool, &command_buffers);
        }

        Ok(())
    }
}
