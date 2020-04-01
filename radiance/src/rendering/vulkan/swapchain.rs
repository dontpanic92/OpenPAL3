use super::adhoc_command_runner::AdhocCommandRunner;
use super::buffer::{Buffer, BufferType};
use super::creation_helpers;
use super::descriptor_managers::DescriptorManager;
use super::image::Image;
use super::image_view::ImageView;
use super::pipeline_manager::PipelineManager;
use super::render_object::VulkanRenderObject;
use super::uniform_buffers::{DynamicUniformBufferManager, PerFrameUniformBuffer};
use crate::rendering::imgui::{vulkan::ImguiVulkanContext, ImguiContext, ImguiFrame};
use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device, Instance};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::{Rc, Weak};

pub struct SwapChain {
    device: Weak<Device>,
    command_pool: vk::CommandPool,
    handle: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    image_views: Vec<ImageView>,
    depth_image: Image,
    depth_image_view: ImageView,
    uniform_buffers: Vec<Buffer>,
    per_frame_descriptor_sets: Vec<vk::DescriptorSet>,
    framebuffers: Vec<vk::Framebuffer>,
    command_buffers: Vec<vk::CommandBuffer>,
    capabilities: vk::SurfaceCapabilitiesKHR,
    pipeline_manager: PipelineManager,
    imgui: ImguiVulkanContext,

    entry: ash::extensions::khr::Swapchain,
}

impl SwapChain {
    pub fn new(
        instance: &Rc<Instance>,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_pool: vk::CommandPool,
        physical_device: vk::PhysicalDevice,
        queue: vk::Queue,
        surface: vk::SurfaceKHR,
        capabilities: vk::SurfaceCapabilitiesKHR,
        format: vk::SurfaceFormatKHR,
        present_mode: vk::PresentModeKHR,
        descriptor_manager: &Rc<DescriptorManager>,
        command_runner: &Rc<AdhocCommandRunner>,
        gui_context: &ImguiContext,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = ash::extensions::khr::Swapchain::new(instance.as_ref(), device.deref());
        let handle = creation_helpers::create_swapchain(
            &entry,
            surface,
            capabilities,
            format,
            present_mode,
        )?;

        let images = unsafe { entry.get_swapchain_images(handle)? };
        let image_views = creation_helpers::create_image_views(&device, &images, format)?;
        let uniform_buffers: Vec<Buffer> = (0..images.len())
            .map(|_| {
                Buffer::new_dynamic_buffer(
                    allocator,
                    BufferType::Uniform,
                    std::mem::size_of::<PerFrameUniformBuffer>(),
                    1,
                )
                .unwrap()
            })
            .collect();

        let mut depth_image = Image::new_depth_image(
            instance,
            physical_device,
            &allocator,
            capabilities.current_extent.width,
            capabilities.current_extent.height,
        )?;

        depth_image.transit_layout(
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            command_runner,
        )?;
        let depth_image_view = ImageView::new_depth_image_view(
            device,
            depth_image.vk_image(),
            depth_image.vk_format(),
        )?;

        descriptor_manager.reset_per_frame_descriptor_pool();
        let pipeline_manager = PipelineManager::new(
            &device,
            &descriptor_manager,
            format.format,
            depth_image.vk_format(),
            capabilities.current_extent,
        );

        let per_frame_descriptor_sets =
            descriptor_manager.allocate_per_frame_descriptor_sets(uniform_buffers.as_slice())?;

        let framebuffers = creation_helpers::create_framebuffers(
            &device,
            &image_views,
            &depth_image_view,
            &capabilities.current_extent,
            pipeline_manager.render_pass().vk_render_pass(),
        )?;

        let command_buffers = {
            let create_info = vk::CommandBufferAllocateInfo::builder()
                .command_pool(command_pool)
                .command_buffer_count(framebuffers.len() as u32)
                .level(vk::CommandBufferLevel::PRIMARY)
                .build();
            unsafe { device.allocate_command_buffers(&create_info)? }
        };

        let imgui = ImguiVulkanContext::new(
            instance,
            physical_device,
            device,
            queue,
            command_pool,
            pipeline_manager.render_pass().vk_render_pass(),
            images.len(),
            gui_context,
        );

        Ok(Self {
            device: Rc::downgrade(device),
            command_pool,
            handle,
            images,
            image_views,
            depth_image,
            depth_image_view,
            uniform_buffers,
            per_frame_descriptor_sets,
            framebuffers,
            command_buffers,
            capabilities,
            pipeline_manager,
            imgui,
            entry,
        })
    }

    pub fn command_buffers(&self) -> &Vec<vk::CommandBuffer> {
        &self.command_buffers
    }

    pub fn imgui_mut(&mut self) -> &mut ImguiVulkanContext {
        &mut self.imgui
    }

    pub fn acquire_next_image(
        &self,
        timeout: u64,
        semaphore: vk::Semaphore,
        fence: vk::Fence,
    ) -> VkResult<(u32, bool)> {
        unsafe {
            self.entry
                .acquire_next_image(self.handle, timeout, semaphore, fence)
        }
    }

    pub fn update_ubo<T>(&mut self, image_index: usize, data: &[T]) {
        self.uniform_buffers[image_index].copy_memory_from(data);
    }

    pub fn present(
        &mut self,
        image_index: u32,
        queue: vk::Queue,
        wait_semaphores: &[vk::Semaphore],
    ) -> VkResult<bool> {
        let swapchains = [self.handle];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHR::builder()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices)
            .build();

        unsafe { self.entry.queue_present(queue, &present_info) }
    }

    pub fn record_command_buffers(
        &mut self,
        image_index: usize,
        objects: &[&VulkanRenderObject],
        dub_manager: &DynamicUniformBufferManager,
        ui_frame: ImguiFrame,
    ) -> Result<vk::CommandBuffer, vk::Result> {
        let device = self.device.upgrade().unwrap();
        let command_buffer = self.command_buffers[image_index];
        let framebuffer = self.framebuffers[image_index];
        let per_frame_descriptor_set = self.per_frame_descriptor_sets[image_index];

        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .build();
        unsafe {
            device.reset_command_buffer(
                command_buffer,
                vk::CommandBufferResetFlags::RELEASE_RESOURCES,
            )?;
            device.begin_command_buffer(command_buffer, &begin_info)?;
        }

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0f32, 0f32, 0f32, 1f32],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.,
                    stencil: 0,
                },
            },
        ];
        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.pipeline_manager.render_pass().vk_render_pass())
            .framebuffer(framebuffer)
            .render_area(
                vk::Rect2D::builder()
                    .offset(vk::Offset2D::builder().x(0).y(0).build())
                    .extent(self.capabilities.current_extent)
                    .build(),
            )
            .clear_values(&clear_values)
            .build();

        unsafe {
            device.cmd_begin_render_pass(
                command_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );
        }

        let mut objects_by_material = HashMap::new();

        for &obj in objects {
            let key = obj.material().name();
            if !objects_by_material.contains_key(key) {
                objects_by_material.insert(key.clone(), vec![]);
            }

            self.pipeline_manager
                .create_pipeline_if_not_exist(obj.material());
            objects_by_material.get_mut(key).unwrap().push(obj);
        }

        for (material_name, object_group) in &objects_by_material {
            let pipeline = self.pipeline_manager.get_pipeline(material_name);

            unsafe {
                device.cmd_bind_pipeline(
                    command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline.vk_pipeline(),
                );

                for obj in object_group {
                    let vertex_buffer = obj.vertex_buffer();
                    let index_buffer = obj.index_buffer();
                    device.cmd_bind_vertex_buffers(
                        command_buffer,
                        0,
                        &[vertex_buffer.vk_buffer()],
                        &[0],
                    );
                    device.cmd_bind_index_buffer(
                        command_buffer,
                        index_buffer.vk_buffer(),
                        0,
                        vk::IndexType::UINT32,
                    );

                    device.cmd_bind_descriptor_sets(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline.pipeline_layout().vk_pipeline_layout(),
                        0,
                        &[
                            per_frame_descriptor_set,
                            dub_manager.descriptor_set(),
                            obj.vk_descriptor_set(),
                        ],
                        &[dub_manager.get_offset(obj.dub_index()) as u32],
                    );
                    device.cmd_draw_indexed(
                        command_buffer,
                        index_buffer.element_count(),
                        1,
                        0,
                        0,
                        0,
                    );
                }
            }
        }

        self.imgui.record_command_buffer(ui_frame, command_buffer);

        unsafe {
            device.cmd_end_render_pass(command_buffer);
            device.end_command_buffer(command_buffer)?;
        }

        Ok(command_buffer)
    }
}

impl Drop for SwapChain {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            for buffer in &self.framebuffers {
                device.destroy_framebuffer(*buffer, None);
            }

            device.free_command_buffers(self.command_pool, &self.command_buffers);
            self.entry.destroy_swapchain(self.handle, None);
        }
    }
}
