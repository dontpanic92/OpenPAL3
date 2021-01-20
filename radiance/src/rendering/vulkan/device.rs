use std::rc::Rc;

use ash::{
    prelude::VkResult,
    vk::{
        Buffer, BufferCopy, BufferImageCopy, BufferMemoryBarrier, CommandBufferBeginInfo,
        CommandBufferResetFlags, CommandPool, CommandPoolCreateInfo, CopyDescriptorSet,
        DependencyFlags, DescriptorPool, DescriptorPoolCreateInfo, DescriptorSet,
        DescriptorSetAllocateInfo, DescriptorSetLayout, DescriptorSetLayoutCreateInfo, DeviceSize,
        Fence, Framebuffer, FramebufferCreateInfo, GraphicsPipelineCreateInfo, Image, ImageLayout,
        ImageMemoryBarrier, ImageView, ImageViewCreateInfo, IndexType, MemoryBarrier,
        PhysicalDevice, Pipeline, PipelineBindPoint, PipelineCache, PipelineLayout,
        PipelineLayoutCreateInfo, PipelineStageFlags, Queue, RenderPass, RenderPassBeginInfo,
        RenderPassCreateInfo, Sampler, SamplerCreateInfo, Semaphore, SemaphoreCreateInfo,
        ShaderModule, ShaderModuleCreateInfo, SubmitInfo, SubpassContents, WriteDescriptorSet,
    },
};
use ash::{
    version::DeviceV1_0,
    vk::{CommandBuffer, CommandBufferAllocateInfo, DescriptorPoolResetFlags},
};

use super::{creation_helpers, instance::Instance};

pub struct Device {
    instance: Rc<Instance>,
    device: ash::Device,
}

impl Device {
    pub fn new(
        instance: Rc<Instance>,
        physical_device: PhysicalDevice,
        graphics_queue_family_index: u32,
    ) -> Self {
        let device = creation_helpers::create_device(
            instance.vk_instance(),
            physical_device,
            graphics_queue_family_index,
        )
        .unwrap();

        Self { instance, device }
    }

    pub fn vk_device(&self) -> &ash::Device {
        &self.device
    }

    pub fn get_device_queue(&self, queue_family_index: u32, queue_index: u32) -> Queue {
        unsafe {
            self.device
                .get_device_queue(queue_family_index, queue_index)
        }
    }

    pub fn create_command_pool(
        &self,
        create_info: &CommandPoolCreateInfo,
    ) -> VkResult<CommandPool> {
        unsafe { self.device.create_command_pool(&create_info, None) }
    }

    pub fn destroy_command_pool(&self, command_pool: CommandPool) {
        unsafe {
            self.device.destroy_command_pool(command_pool, None);
        }
    }

    pub fn create_descriptor_pool(
        &self,
        create_info: &DescriptorPoolCreateInfo,
    ) -> VkResult<DescriptorPool> {
        unsafe { self.device.create_descriptor_pool(&create_info, None) }
    }

    pub fn create_descriptor_set_layout(
        &self,
        create_info: &DescriptorSetLayoutCreateInfo,
    ) -> VkResult<DescriptorSetLayout> {
        unsafe { self.device.create_descriptor_set_layout(&create_info, None) }
    }

    pub fn allocate_descriptor_sets(
        &self,
        create_info: &DescriptorSetAllocateInfo,
    ) -> VkResult<Vec<DescriptorSet>> {
        unsafe { self.device.allocate_descriptor_sets(&create_info) }
    }

    pub fn update_descriptor_sets(
        &self,
        write_descriptor_sets: &[WriteDescriptorSet],
        copy_descriptor_sets: &[CopyDescriptorSet],
    ) {
        unsafe {
            self.device
                .update_descriptor_sets(write_descriptor_sets, copy_descriptor_sets)
        }
    }

    pub fn reset_descriptor_pool(&self, pool: DescriptorPool) -> VkResult<()> {
        unsafe {
            self.device
                .reset_descriptor_pool(pool, DescriptorPoolResetFlags::empty())
        }
    }

    pub fn destroy_descriptor_set_layout(&self, descriptor_set_layout: DescriptorSetLayout) {
        unsafe {
            self.device
                .destroy_descriptor_set_layout(descriptor_set_layout, None);
        }
    }

    pub fn destroy_descriptor_pool(&self, descriptor_pool: DescriptorPool) {
        unsafe {
            self.device.destroy_descriptor_pool(descriptor_pool, None);
        }
    }

    pub fn allocate_command_buffers(
        &self,
        allocation_info: &CommandBufferAllocateInfo,
    ) -> VkResult<Vec<CommandBuffer>> {
        unsafe { self.device.allocate_command_buffers(&allocation_info) }
    }

    pub fn begin_command_buffer(
        &self,
        command_buffer: CommandBuffer,
        begin_info: &CommandBufferBeginInfo,
    ) -> VkResult<()> {
        unsafe {
            self.device
                .begin_command_buffer(command_buffer, &begin_info)
        }
    }

    pub fn cmd_copy_buffer(
        &self,
        command_buffer: CommandBuffer,
        src_buffer: Buffer,
        dst_buffer: Buffer,
        regions: &[BufferCopy],
    ) {
        unsafe {
            self.device
                .cmd_copy_buffer(command_buffer, src_buffer, dst_buffer, regions);
        }
    }

    pub fn end_command_buffer(&self, command_buffer: CommandBuffer) -> VkResult<()> {
        unsafe { self.device.end_command_buffer(command_buffer) }
    }

    pub fn reset_command_buffer(
        &self,
        command_buffer: CommandBuffer,
        flags: CommandBufferResetFlags,
    ) -> VkResult<()> {
        unsafe { self.device.reset_command_buffer(command_buffer, flags) }
    }

    pub fn cmd_pipeline_barrier(
        &self,
        command_buffer: CommandBuffer,
        src_stage_mask: PipelineStageFlags,
        dst_stage_mask: PipelineStageFlags,
        dependency_flags: DependencyFlags,
        memory_barriers: &[MemoryBarrier],
        buffer_memory_barriers: &[BufferMemoryBarrier],
        image_memory_barriers: &[ImageMemoryBarrier],
    ) {
        unsafe {
            self.device.cmd_pipeline_barrier(
                command_buffer,
                src_stage_mask,
                dst_stage_mask,
                dependency_flags,
                memory_barriers,
                buffer_memory_barriers,
                image_memory_barriers,
            )
        }
    }

    pub fn cmd_copy_buffer_to_image(
        &self,
        command_buffer: CommandBuffer,
        src_buffer: Buffer,
        dst_image: Image,
        dst_image_layout: ImageLayout,
        regions: &[BufferImageCopy],
    ) {
        unsafe {
            self.device.cmd_copy_buffer_to_image(
                command_buffer,
                src_buffer,
                dst_image,
                dst_image_layout,
                regions,
            )
        }
    }

    pub fn cmd_begin_render_pass(
        &self,
        command_buffer: CommandBuffer,
        create_info: &RenderPassBeginInfo,
        contents: SubpassContents,
    ) {
        unsafe {
            self.device
                .cmd_begin_render_pass(command_buffer, create_info, contents);
        }
    }

    pub fn cmd_end_render_pass(&self, command_buffer: CommandBuffer) {
        unsafe { self.device.cmd_end_render_pass(command_buffer) }
    }

    pub fn cmd_bind_pipeline(
        &self,
        command_buffer: CommandBuffer,
        pipeline_bind_point: PipelineBindPoint,
        pipeline: Pipeline,
    ) {
        unsafe {
            self.device
                .cmd_bind_pipeline(command_buffer, pipeline_bind_point, pipeline);
        }
    }

    pub fn cmd_bind_vertex_buffers(
        &self,
        command_buffer: CommandBuffer,
        first_binding: u32,
        buffers: &[Buffer],
        offsets: &[DeviceSize],
    ) {
        unsafe {
            self.device
                .cmd_bind_vertex_buffers(command_buffer, first_binding, buffers, offsets);
        }
    }

    pub fn cmd_bind_index_buffer(
        &self,
        command_buffer: CommandBuffer,
        buffer: Buffer,
        offset: DeviceSize,
        index_type: IndexType,
    ) {
        unsafe {
            self.device
                .cmd_bind_index_buffer(command_buffer, buffer, offset, index_type);
        }
    }

    pub fn cmd_bind_descriptor_sets(
        &self,
        command_buffer: CommandBuffer,
        pipeline_bind_point: PipelineBindPoint,
        layout: PipelineLayout,
        first_set: u32,
        descriptor_sets: &[DescriptorSet],
        dynamic_offsets: &[u32],
    ) {
        unsafe {
            self.device.cmd_bind_descriptor_sets(
                command_buffer,
                pipeline_bind_point,
                layout,
                first_set,
                descriptor_sets,
                dynamic_offsets,
            );
        }
    }

    pub fn cmd_draw_indexed(
        &self,
        command_buffer: CommandBuffer,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        unsafe {
            self.device.cmd_draw_indexed(
                command_buffer,
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            );
        }
    }

    pub fn queue_submit(&self, queue: Queue, submits: &[SubmitInfo], fence: Fence) -> VkResult<()> {
        unsafe { self.device.queue_submit(queue, submits, fence) }
    }

    pub fn queue_wait_idle(&self, queue: Queue) -> VkResult<()> {
        unsafe { self.device.queue_wait_idle(queue) }
    }

    pub fn free_command_buffers(
        &self,
        command_pool: CommandPool,
        command_buffers: &[CommandBuffer],
    ) {
        unsafe {
            self.device
                .free_command_buffers(command_pool, &command_buffers)
        }
    }

    pub fn create_image_view(&self, create_info: &ImageViewCreateInfo) -> VkResult<ImageView> {
        unsafe { self.device.create_image_view(&create_info, None) }
    }

    pub fn destroy_image_view(&self, image_view: ImageView) {
        unsafe {
            self.device.destroy_image_view(image_view, None);
        }
    }

    pub fn create_render_pass(&self, create_info: &RenderPassCreateInfo) -> VkResult<RenderPass> {
        unsafe { self.device.create_render_pass(&create_info, None) }
    }

    pub fn destroy_render_pass(&self, render_pass: RenderPass) {
        unsafe { self.device.destroy_render_pass(render_pass, None) }
    }

    pub fn create_graphics_pipelines(
        &self,
        create_infos: &[GraphicsPipelineCreateInfo],
    ) -> Result<Vec<Pipeline>, (Vec<Pipeline>, ash::vk::Result)> {
        unsafe {
            self.device
                .create_graphics_pipelines(PipelineCache::default(), create_infos, None)
        }
    }

    pub fn destroy_pipeline(&self, pipeline: Pipeline) {
        unsafe { self.device.destroy_pipeline(pipeline, None) }
    }

    pub fn create_pipeline_layout(
        &self,
        create_info: &PipelineLayoutCreateInfo,
    ) -> VkResult<PipelineLayout> {
        unsafe { self.device.create_pipeline_layout(&create_info, None) }
    }

    pub fn destroy_pipeline_layout(&self, pipeline_layout: PipelineLayout) {
        unsafe { self.device.destroy_pipeline_layout(pipeline_layout, None) }
    }

    pub fn create_sampler(&self, create_info: &SamplerCreateInfo) -> VkResult<Sampler> {
        unsafe { self.device.create_sampler(&create_info, None) }
    }

    pub fn destroy_sampler(&self, sampler: Sampler) {
        unsafe { self.device.destroy_sampler(sampler, None) }
    }

    pub fn create_shader_module(
        &self,
        create_info: &ShaderModuleCreateInfo,
    ) -> VkResult<ShaderModule> {
        unsafe { self.device.create_shader_module(&create_info, None) }
    }

    pub fn destroy_shader_module(&self, shader: ShaderModule) {
        unsafe { self.device.destroy_shader_module(shader, None) }
    }

    pub fn create_framebuffer(&self, create_info: &FramebufferCreateInfo) -> VkResult<Framebuffer> {
        unsafe { self.device.create_framebuffer(&create_info, None) }
    }

    pub fn destroy_framebuffer(&self, framebuffer: Framebuffer) {
        unsafe { self.device.destroy_framebuffer(framebuffer, None) }
    }

    pub fn create_semaphore(&self, create_info: &SemaphoreCreateInfo) -> VkResult<Semaphore> {
        unsafe { self.device.create_semaphore(create_info, None) }
    }

    pub fn destroy_semaphore(&self, semaphore: Semaphore) {
        unsafe {
            self.device.destroy_semaphore(semaphore, None);
        }
    }

    pub fn wait_idle(&self) {
        unsafe { self.device.device_wait_idle().unwrap() }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);
        }
    }
}
