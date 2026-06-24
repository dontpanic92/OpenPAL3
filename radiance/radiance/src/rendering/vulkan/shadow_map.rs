//! Directional shadow map (single sun, one depth pass + PCF).
//!
//! Owns a fixed-resolution depth render target rendered from the sun's point
//! of view each frame, plus the depth-only pipelines (one per caster vertex
//! stride) that write it and a clamp-to-white-border sampler the lit fragment
//! shaders read it through. Resolution-independent, so it is allocated once at
//! engine init and survives swapchain recreation.
//!
//! The light-space matrix ([`light_view_proj`]) is computed per frame on the
//! CPU and uploaded in the per-frame UBO (`PerFrameUniformBuffer::set_shadow`),
//! shared by both the depth pass (`shadow_depth.vert`) and the sampling
//! shaders so the projection stays consistent.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ash::vk;

use crate::math::{Mat44, Transform, Vec3};
use crate::rendering::ShaderProgram;
use crate::rendering::vertex_buffer::{VertexComponents, VertexComponentsLayout};

use super::descriptor_managers::DescriptorManager;
use super::device::Device;
use super::image::Image;
use super::image_view::ImageView;
use super::instance::Instance;

/// Square shadow-map resolution. 2048² balances coverage vs. memory for the
/// single-map scope.
pub const SHADOW_MAP_SIZE: u32 = 2048;

/// Depth-compare bias (in light-space `[0,1]` depth units) used by the lit
/// shaders to fight shadow acne. Paired with the pipeline's slope-scaled depth
/// bias on the write side.
pub const SHADOW_BIAS: f32 = 0.0015;

/// PCF kernel radius in texels (3×3 → radius 1).
pub const SHADOW_PCF_RADIUS: f32 = 1.0;

static SHADOW_DEPTH_VERT: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/shadow_depth.vert.spv"));

/// One depth-only pipeline, specialized to a caster vertex stride.
pub struct ShadowPipeline {
    device: Rc<Device>,
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}

impl Drop for ShadowPipeline {
    fn drop(&mut self) {
        self.device.destroy_pipeline(self.pipeline);
        self.device.destroy_pipeline_layout(self.layout);
    }
}

pub struct ShadowMap {
    device: Rc<Device>,

    _depth_image: Image,
    depth_view: ImageView,
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    sampler: vk::Sampler,

    vert_module: vk::ShaderModule,
    /// Depth pipelines keyed by caster vertex stride (one per distinct
    /// `ShaderProgram` vertex layout; POSITION is always at offset 0).
    pipelines: RefCell<HashMap<u32, Rc<ShadowPipeline>>>,
}

impl ShadowMap {
    pub fn new(
        device: Rc<Device>,
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        allocator: &Rc<vk_mem::Allocator>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let depth_image = Image::new_depth_sampled_image(
            &instance.vk_instance(),
            physical_device,
            allocator,
            SHADOW_MAP_SIZE,
            SHADOW_MAP_SIZE,
        )?;
        let depth_format = depth_image.vk_format();
        let depth_view = ImageView::new_depth_image_view(
            device.clone(),
            depth_image.vk_image(),
            depth_format,
        )?;

        let render_pass = create_shadow_render_pass(&device, depth_format)?;

        let attachments = [depth_view.vk_image_view()];
        let fb_info = vk::FramebufferCreateInfo::default()
            .render_pass(render_pass)
            .attachments(&attachments)
            .layers(1)
            .width(SHADOW_MAP_SIZE)
            .height(SHADOW_MAP_SIZE);
        let framebuffer = device.create_framebuffer(&fb_info)?;

        let sampler = super::descriptor_managers::create_shadow_sampler(&device)?;

        let vert_module = create_shader_module(&device, SHADOW_DEPTH_VERT)?;

        Ok(Self {
            device,
            _depth_image: depth_image,
            depth_view,
            render_pass,
            framebuffer,
            sampler,
            vert_module,
            pipelines: RefCell::new(HashMap::new()),
        })
    }

    pub fn image_view(&self) -> vk::ImageView {
        self.depth_view.vk_image_view()
    }

    pub fn sampler(&self) -> vk::Sampler {
        self.sampler
    }

    pub fn render_pass(&self) -> vk::RenderPass {
        self.render_pass
    }

    pub fn framebuffer(&self) -> vk::Framebuffer {
        self.framebuffer
    }

    pub fn extent(&self) -> vk::Extent2D {
        vk::Extent2D {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
        }
    }

    /// Get (creating on first use) the depth pipeline for a caster
    /// `ShaderProgram`. Keyed by the program's vertex stride: every program
    /// keeps POSITION first at offset 0, so two programs with the same stride
    /// share a pipeline.
    pub fn pipeline_for(
        &self,
        program: ShaderProgram,
        descriptor_manager: &DescriptorManager,
    ) -> Rc<ShadowPipeline> {
        let stride = VertexComponentsLayout::from_components(program_components(program)).size()
            as u32;
        if let Some(p) = self.pipelines.borrow().get(&stride) {
            return p.clone();
        }
        let pipeline = Rc::new(self.create_pipeline(stride, descriptor_manager));
        self.pipelines.borrow_mut().insert(stride, pipeline.clone());
        pipeline
    }

    fn create_pipeline(
        &self,
        stride: u32,
        descriptor_manager: &DescriptorManager,
    ) -> ShadowPipeline {
        let set_layouts = [
            descriptor_manager.per_frame_layout(),
            descriptor_manager.dub_layout(),
        ];
        let layout_info = vk::PipelineLayoutCreateInfo::default().set_layouts(&set_layouts);
        let layout = self.device.create_pipeline_layout(&layout_info).unwrap();

        let entry = std::ffi::CString::new("main").unwrap();
        let stages = [vk::PipelineShaderStageCreateInfo::default()
            .name(&entry)
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(self.vert_module)];

        let binding_descriptions = [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(stride)
            .input_rate(vk::VertexInputRate::VERTEX)];
        let attribute_descriptions = [vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(0)
            .offset(0)
            .format(vk::Format::R32G32B32_SFLOAT)];
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        // Static viewport/scissor at the shadow-map size — no dynamic state, so
        // the depth pass must not call `cmd_set_viewport`.
        let viewport = vk::Viewport::default()
            .x(0.0)
            .y(0.0)
            .width(SHADOW_MAP_SIZE as f32)
            .height(SHADOW_MAP_SIZE as f32)
            .min_depth(0.0)
            .max_depth(1.0);
        let scissor = vk::Rect2D::default()
            .offset(vk::Offset2D::default())
            .extent(self.extent());
        let viewports = [viewport];
        let scissors = [scissor];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);

        // Front-face culling + slope-scaled depth bias to reduce acne /
        // peter-panning. `front_face` matches the scene pipelines.
        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::FRONT)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(true)
            .depth_bias_constant_factor(2.0)
            .depth_bias_slope_factor(2.5)
            .depth_bias_clamp(0.0);

        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        // No color attachments in the depth-only pass.
        let color_blend = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(&[]);

        let create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization)
            .multisample_state(&multisample)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blend)
            .layout(layout)
            .render_pass(self.render_pass)
            .subpass(0);

        let pipeline = self
            .device
            .create_graphics_pipelines(&[create_info])
            .map_err(|(_, e)| e)
            .expect("failed to create shadow depth pipeline")[0];

        ShadowPipeline {
            device: self.device.clone(),
            pipeline,
            layout,
        }
    }
}

impl ShadowPipeline {
    pub fn vk_pipeline(&self) -> vk::Pipeline {
        self.pipeline
    }

    pub fn vk_layout(&self) -> vk::PipelineLayout {
        self.layout
    }
}

impl Drop for ShadowMap {
    fn drop(&mut self) {
        self.pipelines.borrow_mut().clear();
        self.device.destroy_shader_module(self.vert_module);
        self.device.destroy_framebuffer(self.framebuffer);
        self.device.destroy_render_pass(self.render_pass);
        self.device.destroy_sampler(self.sampler);
    }
}

fn program_components(program: ShaderProgram) -> VertexComponents {
    match program {
        ShaderProgram::TexturedNoLight => VertexComponents::POSITION | VertexComponents::TEXCOORD,
        ShaderProgram::TexturedLightmap => {
            VertexComponents::POSITION | VertexComponents::TEXCOORD | VertexComponents::TEXCOORD2
        }
        ShaderProgram::GradientY => VertexComponents::POSITION | VertexComponents::TEXCOORD,
        ShaderProgram::TexturedDynamicLit => {
            VertexComponents::POSITION | VertexComponents::NORMAL | VertexComponents::TEXCOORD
        }
        ShaderProgram::TerrainSplat => {
            VertexComponents::POSITION | VertexComponents::NORMAL | VertexComponents::TEXCOORD
        }
    }
}

fn create_shadow_render_pass(
    device: &Rc<Device>,
    depth_format: vk::Format,
) -> ash::prelude::VkResult<vk::RenderPass> {
    let depth_attachment = vk::AttachmentDescription::default()
        .format(depth_format)
        .samples(vk::SampleCountFlags::TYPE_1)
        .load_op(vk::AttachmentLoadOp::CLEAR)
        .store_op(vk::AttachmentStoreOp::STORE)
        .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
        .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .final_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);

    let depth_reference = vk::AttachmentReference::default()
        .attachment(0)
        .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

    let subpass = vk::SubpassDescription::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .depth_stencil_attachment(&depth_reference);

    // Order vs. the previous frame's sampling (external → subpass) and the
    // upcoming main pass's sampling (subpass → external).
    let dependencies = [
        vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .src_access_mask(vk::AccessFlags::SHADER_READ)
            .dst_stage_mask(vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .dst_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE),
        vk::SubpassDependency::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .src_stage_mask(vk::PipelineStageFlags::LATE_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE)
            .dst_stage_mask(vk::PipelineStageFlags::FRAGMENT_SHADER)
            .dst_access_mask(vk::AccessFlags::SHADER_READ),
    ];

    let attachments = [depth_attachment];
    let subpasses = [subpass];
    let create_info = vk::RenderPassCreateInfo::default()
        .attachments(&attachments)
        .subpasses(&subpasses)
        .dependencies(&dependencies);

    device.create_render_pass(&create_info)
}

fn create_shader_module(
    device: &Rc<Device>,
    code: &[u8],
) -> ash::prelude::VkResult<vk::ShaderModule> {
    // SPIR-V must be `u32`-aligned; copy into an owned Vec to guarantee it
    // (mirrors `VulkanShader::create_shader_module_from_memory`).
    assert!(code.len() % 4 == 0, "SPIR-V blob length must be /4");
    let mut words = vec![0u32; code.len() / 4];
    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), words.as_mut_ptr().cast::<u8>(), code.len());
    }
    let create_info = vk::ShaderModuleCreateInfo::default().code(&words);
    device.create_shader_module(&create_info)
}

/// Compute the sun's light-space view-projection (world → shadow clip space)
/// with the Vulkan clip (Y-flip + `[0,1]` depth) remap folded in, matching the
/// scene shaders' row-vector (`v * M`) convention.
///
/// The orthographic frustum is centered on `focus` (the visible casters' AABB
/// center) with the given `half_extent` (half the visible casters' horizontal
/// span), and looks from `focus + sun_dir * light_distance` back toward
/// `focus`. The light distance and far plane scale with `half_extent` so the
/// frustum always contains casters above/below the focus along the (often
/// near-horizontal) sun direction. `sun_dir` is the unit direction from a
/// surface **toward** the sun.
pub fn light_view_proj(focus: Vec3, sun_dir: Vec3, half_extent: f32) -> Mat44 {
    let sun = Vec3::normalized(&sun_dir);
    // Push the eye back far enough (relative to coverage) that the whole fit
    // box stays in front of the near plane even for a grazing sun.
    let light_distance = half_extent * 4.0;
    let near = 1.0;
    let far = half_extent * 8.0;
    let eye = Vec3::add(&focus, &Vec3::scalar_mul(light_distance, &sun));

    let mut light = Transform::new();
    light.set_position(&eye);
    light.look_at(&focus);
    let view = Mat44::inversed(light.matrix());

    let proj = ortho(half_extent, near, far);

    // Vulkan clip remap (GL `[-1,1]` z → `[0,1]`, flip Y), applied as `C * v`.
    let mut clip = Mat44::new_identity();
    clip[1][1] = -1.0;
    clip[2][2] = 0.5;
    clip[2][3] = 0.5;

    // GLSL `world * M` evaluates as `M_rust * world`, so the composed matrix is
    // `clip * proj * view` (applies view first).
    Mat44::multiplied(&clip, &Mat44::multiplied(&proj, &view))
}

/// Standard OpenGL symmetric orthographic projection (column-vector / `M * v`
/// convention), box `[-h, h]²` in XY and `[near, far]` along `-Z`.
fn ortho(half_extent: f32, near: f32, far: f32) -> Mat44 {
    let mut m = Mat44::new_zero();
    m[0][0] = 1.0 / half_extent;
    m[1][1] = 1.0 / half_extent;
    m[2][2] = -2.0 / (far - near);
    m[2][3] = -(far + near) / (far - near);
    m[3][3] = 1.0;
    m
}
