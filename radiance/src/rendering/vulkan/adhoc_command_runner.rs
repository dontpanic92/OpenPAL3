use ash::prelude::VkResult;
use ash::vk;
use std::rc::Rc;

use super::device::Device;

pub struct AdhocCommandRunner {
    device: Rc<Device>,
    command_pool: vk::CommandPool,
    queue: vk::Queue,
}

impl AdhocCommandRunner {
    pub fn new(device: Rc<Device>, command_pool: vk::CommandPool, queue: vk::Queue) -> Self {
        Self {
            device,
            command_pool,
            queue,
        }
    }

    pub fn run_commands_one_shot<F: FnOnce(&Rc<Device>, &vk::CommandBuffer)>(
        &self,
        record_command: F,
    ) -> VkResult<()> {
        let command_buffers = {
            let allocation_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(self.command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1)
                .build();
            self.device.allocate_command_buffers(&allocation_info)?
        };

        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        self.device
            .begin_command_buffer(command_buffers[0], &begin_info)?;

        record_command(&self.device, &command_buffers[0]);
        self.device.end_command_buffer(command_buffers[0])?;

        let submit_info = vk::SubmitInfo::builder()
            .command_buffers(&command_buffers)
            .build();
        self.device
            .queue_submit(self.queue, &[submit_info], vk::Fence::default())?;

        self.device.queue_wait_idle(self.queue)?;
        self.device
            .free_command_buffers(self.command_pool, &command_buffers);

        Ok(())
    }
}
