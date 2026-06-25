use super::buffer::{Buffer, BufferType};
use super::shadow_map::CASCADE_COUNT;
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

    /// Number of UBO slots currently marked in-use. Walks the usage
    /// bitmask (O(pool_size)), so cheap-but-not-free — callers should
    /// only invoke this once per frame at most. Returns a snapshot;
    /// concurrent allocations or releases will not be observed.
    pub fn allocated_slots(&self) -> usize {
        let usage = self.usage.lock().unwrap();
        usage.iter().filter(|&&u| u).count()
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
    /// `xyz` = ambient color, `w` = active light count (as f32).
    ambient: [f32; 4],
    /// World-space light positions (`xyz`; `w` = outer attenuation radius).
    /// Only the first `ambient.w` entries are meaningful.
    light_pos: [[f32; 4]; MAX_SCENE_LIGHTS],
    /// Light colors (`rgb`; `w` = inner attenuation radius), paired with
    /// `light_pos`.
    light_color: [[f32; 4]; MAX_SCENE_LIGHTS],
    /// Directional sun light. `xyz` = unit direction from surface toward the
    /// sun; `w` = enabled flag (`1.0` when a sun is present, else `0.0`).
    sun_dir: [f32; 4],
    /// Directional sun color (`rgb`; `w` reserved).
    sun_color: [f32; 4],
    /// Per-cascade sun light-space view-projections (world → shadow-map clip
    /// space), Vulkan clip remap folded in. Meaningful only when
    /// `shadow_params.x >= 0.5`. Index with the cascade selected by view-space
    /// depth.
    light_view_proj: [Mat44; CASCADE_COUNT],
    /// View-space far-edge depth of each cascade (positive, increasing). The
    /// lit shaders pick the first cascade whose split `>=` the fragment's
    /// view-space depth. Lane `w` is spare.
    cascade_splits: [f32; 4],
    /// Shadow sampling parameters: `x` = enabled flag (`1.0`/`0.0`),
    /// `y` = depth-compare bias, `z` = shadow-map texel size (`1 / size`),
    /// `w` = PCF kernel radius (in texels).
    shadow_params: [f32; 4],
    /// Linear distance fog color (`rgb`; `w` reserved). Meaningful only when
    /// `fog_params.x >= 0.5`.
    fog_color: [f32; 4],
    /// Linear distance fog parameters: `x` = enabled flag (`1.0`/`0.0`),
    /// `y` = fog start (view-space depth), `z` = fog end (view-space depth),
    /// `w` reserved.
    fog_params: [f32; 4],
}

/// Maximum number of scene point lights uploaded per frame. PAL3 scenes ship
/// at most 16 lights; the dynamically-lit shader loops over `ambient.w` of
/// these.
pub const MAX_SCENE_LIGHTS: usize = 16;

impl PerFrameUniformBuffer {
    pub fn new(view: &Mat44, projection: &Mat44) -> Self {
        Self {
            view: *view,
            projection: *projection,
            ambient: [0.0; 4],
            light_pos: [[0.0; 4]; MAX_SCENE_LIGHTS],
            light_color: [[0.0; 4]; MAX_SCENE_LIGHTS],
            sun_dir: [0.0; 4],
            sun_color: [0.0; 4],
            light_view_proj: [Mat44::new_identity(); CASCADE_COUNT],
            cascade_splits: [0.0; 4],
            shadow_params: [0.0; 4],
            fog_color: [0.0; 4],
            fog_params: [0.0; 4],
        }
    }

    /// Stamp the ambient term and up to [`MAX_SCENE_LIGHTS`] point lights
    /// (position + color + `[inner, outer]` attenuation range) into the
    /// per-frame UBO. The range is packed into the otherwise-unused `w` lanes:
    /// outer radius in `light_pos.w`, inner radius in `light_color.w`.
    pub fn set_lighting(&mut self, ambient: [f32; 3], lights: &[([f32; 3], [f32; 3], [f32; 2])]) {
        let count = lights.len().min(MAX_SCENE_LIGHTS);
        self.ambient = [ambient[0], ambient[1], ambient[2], count as f32];
        for (i, (pos, color, range)) in lights.iter().take(count).enumerate() {
            let inner = range[0];
            let outer = range[1];
            self.light_pos[i] = [pos[0], pos[1], pos[2], outer];
            self.light_color[i] = [color[0], color[1], color[2], inner];
        }
    }

    /// Stamp the directional sun light into the per-frame UBO. `direction` is
    /// the unit vector from a surface toward the sun; `color` is its linear
    /// RGB intensity. Passing `None` disables the directional term
    /// (`sun_dir.w = 0`).
    pub fn set_sun(&mut self, sun: Option<([f32; 3], [f32; 3])>) {
        match sun {
            Some((dir, color)) => {
                self.sun_dir = [dir[0], dir[1], dir[2], 1.0];
                self.sun_color = [color[0], color[1], color[2], 0.0];
            }
            None => {
                self.sun_dir = [0.0; 4];
                self.sun_color = [0.0; 4];
            }
        }
    }

    /// Stamp the directional CSM's per-cascade light-space matrices, cascade
    /// split depths, and sampling parameters into the per-frame UBO.
    /// `Some((matrices, splits, params))` enables shadow sampling; `None`
    /// disables it (`shadow_params.x = 0`), leaving the matrices at identity.
    /// `params` lanes: `[enabled, bias, 1/size, pcf_radius]` (the supplied
    /// `enabled` lane is overwritten with `1.0` here).
    pub fn set_shadow(
        &mut self,
        shadow: Option<([Mat44; CASCADE_COUNT], [f32; CASCADE_COUNT], [f32; 4])>,
    ) {
        match shadow {
            Some((matrices, splits, params)) => {
                self.light_view_proj = matrices;
                self.cascade_splits = [0.0; 4];
                for (i, s) in splits.iter().enumerate() {
                    self.cascade_splits[i] = *s;
                }
                self.shadow_params = [1.0, params[1], params[2], params[3]];
            }
            None => {
                self.light_view_proj = [Mat44::new_identity(); CASCADE_COUNT];
                self.cascade_splits = [0.0; 4];
                self.shadow_params = [0.0; 4];
            }
        }
    }

    /// Stamp linear distance fog into the per-frame UBO. `Some((color, start,
    /// end))` enables fog sampling in the world-geometry shaders (`fog_params.x
    /// = 1`); `None` disables it (`fog_params.x = 0`). `start`/`end` are
    /// view-space depths (positive, forward).
    pub fn set_fog(&mut self, fog: Option<([f32; 3], f32, f32)>) {
        match fog {
            Some((color, start, end)) => {
                self.fog_color = [color[0], color[1], color[2], 0.0];
                self.fog_params = [1.0, start, end, 0.0];
            }
            None => {
                self.fog_color = [0.0; 4];
                self.fog_params = [0.0; 4];
            }
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
/// - `misc.z`   — `ambient_floor` (lightmap additive term)
/// - `misc.w`   — `fog_exempt` flag (`1.0` = skip scene fog for this material)
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
            misc: [
                p.alpha_ref,
                p.intensity,
                p.ambient_floor,
                if p.fog_exempt { 1.0 } else { 0.0 },
            ],
            uv_xform: [p.uv_scale[0], p.uv_scale[1], p.uv_offset[0], p.uv_offset[1]],
        }
    }
}
