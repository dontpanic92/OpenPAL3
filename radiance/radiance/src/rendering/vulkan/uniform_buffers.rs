use super::buffer::{Buffer, BufferType};
use crate::math::Mat44;
use crate::rendering::vulkan::descriptor_managers::DynamicUniformBufferDescriptorManager;
use ash::vk;
use std::{cell::RefCell, rc::Rc, sync::Mutex};

#[derive(Clone)]
#[repr(C)]
pub struct PerInstanceUniformBuffer {
    model: Mat44,
}

impl PerInstanceUniformBuffer {
    pub fn _new(model: &Mat44) -> Self {
        Self { model: *model }
    }
}

pub struct DynamicUniformBufferManager {
    descriptor_set: vk::DescriptorSet,
    alignment: u64,
    buffer: RefCell<Buffer>,
    usage: Mutex<Vec<bool>>,
}

impl DynamicUniformBufferManager {
    /// Per-instance UBO pool size. Each [`VulkanRenderObject`] takes
    /// one slot and releases it on drop, so the cap is roughly
    /// `max_render_objects_live_at_any_one_moment`.
    ///
    /// PAL4 scenes are big (~1500 render objects each), and during
    /// the brief window where a new scene is built before the old
    /// `Pal4Scene` is dropped, two scenes' worth of UBOs are live
    /// simultaneously. With the previous cap of 10240 the planner
    /// reliably reached ~7 scene transitions before the
    /// `allocate_buffer` panic — small enough that PAL4 chapter
    /// progression hit the wall mid-plot (e.g. Q01 → M01 → M02 →
    /// Q02 → M03 → Q03 panicked on the Q03 BSP load).
    ///
    /// 65536 gives us headroom for the full PAL4 playthrough and
    /// still costs only ~16 MB of GPU memory (each slot is
    /// `min_uniform_buffer_offset_alignment` ≈ 256 bytes). The
    /// underlying leak (something is holding the old `Pal4Scene`'s
    /// entities longer than expected) is unfixed; this is a
    /// pragmatic cap bump, not a real fix.
    const POOL_SIZE: u32 = 65536;

    pub fn new(
        allocator: &Rc<vk_mem::Allocator>,
        descriptor_manager: &DynamicUniformBufferDescriptorManager,
        min_alignment: u64,
    ) -> Self {
        let (alignment, buffer) =
            Self::allocate_dynamic_buffer(allocator, min_alignment, Self::POOL_SIZE);
        let descriptor_set = descriptor_manager.allocate_descriptor_sets(&buffer);

        Self {
            descriptor_set,
            alignment,
            buffer: RefCell::new(buffer),
            usage: Mutex::new(vec![false; Self::POOL_SIZE as usize]),
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
        self.buffer.borrow_mut().map_memory_do(|dst| {
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

/// GPU-side layout of [`crate::rendering::MaterialParams`] used by the
/// per-material UBO (descriptor set 3). All members are `vec4`s for
/// std140 friendliness. Lane meanings:
///
/// - `tint`     — RGB tint and (premultiplied) alpha multiplier
/// - `misc.x`   — `alpha_ref` (consulted only when `ALPHA_TEST` is set)
/// - `misc.y`   — `intensity` (lightmap scalar; pass-through for non-
///                lightmap shaders that simply ignore it)
/// - `misc.zw`  — reserved
/// - `uv_xform` — primary-UV affine: xy = scale, zw = offset
#[repr(C)]
#[derive(Copy, Clone)]
pub struct MaterialParamsGpu {
    pub tint: [f32; 4],
    pub misc: [f32; 4],
    pub uv_xform: [f32; 4],
}

impl MaterialParamsGpu {
    pub fn from_params(p: &crate::rendering::MaterialParams) -> Self {
        Self {
            tint: p.tint,
            misc: [p.alpha_ref, p.intensity, 0.0, 0.0],
            uv_xform: [p.uv_scale[0], p.uv_scale[1], p.uv_offset[0], p.uv_offset[1]],
        }
    }
}
