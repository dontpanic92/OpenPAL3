use super::buffer::{Buffer, BufferType};
use super::material::VulkanMaterial;
use super::uniform_buffers::DynamicUniformBufferManager;
use crate::math::{Mat44, Vec3};
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::vulkan::descriptor_managers::DescriptorManager;
use crate::rendering::{RenderObject, ShaderProgram, VertexBuffer};
use ash::vk;
use std::cell::{Cell, OnceCell, Ref, RefCell, RefMut};
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;

pub struct VulkanRenderObject {
    vertices: RefCell<VertexBuffer>,
    _indices: Vec<u32>,
    host_dynamic: bool,
    vertex_revision: Cell<u64>,

    dub_manager: Arc<DynamicUniformBufferManager>,
    descriptor_manager: Rc<DescriptorManager>,
    vertex_buffer: Option<Buffer>,
    index_buffer: Buffer,
    material: Rc<VulkanMaterial>,
    camera_clip: OnceCell<CameraClip>,
    per_object_descriptor_set: vk::DescriptorSet,
    dub_index: usize,
    /// Second dynamic-UBO slot holding this object's *shadow* model matrix.
    /// Identical to `dub_index`'s model for ordinary objects, but billboards
    /// write a camera-independent (flat) matrix here so cast shadows don't
    /// yaw with the camera. The shadow depth pass binds this slot.
    shadow_dub_index: usize,
    /// Local-space bounds of the *current* vertex data. Recomputed
    /// whenever [`RenderObject::update_vertices`] rewrites the buffer:
    /// CPU-skinned meshes deform their positions every frame, so a
    /// construction-time (bind-pose) snapshot would make the frustum
    /// culler drop posed limbs that moved outside the rest pose (e.g. a
    /// character's head vanishing while it sits on a bed).
    local_centroid: Cell<[f32; 3]>,
    local_aabb: Cell<Option<([f32; 3], [f32; 3])>>,
}

struct CameraClip {
    center: [f32; 3],
    distance: f32,
    opacity: Cell<f32>,
    enabled: Cell<bool>,
    material: Rc<VulkanMaterial>,
}

impl RenderObject for VulkanRenderObject {
    fn set_camera_clip(&self, clip: Option<([f32; 3], f32)>) -> bool {
        let Some((center, distance)) = clip else {
            if let Some(clip) = self.camera_clip.get() {
                clip.enabled.set(false);
                clip.opacity.set(1.);
            }
            return true;
        };
        if !center.iter().all(|v| v.is_finite())
            || !distance.is_finite()
            || distance < 0.
            || self.material.key().program != ShaderProgram::TexturedNoLight
        {
            log::debug!(
                "Camera clipping rejected for material {} ({:?})",
                self.material.debug_name(),
                self.material.key()
            );
            return false;
        }
        let clip = self.camera_clip.get_or_init(|| CameraClip {
            center,
            distance,
            opacity: Cell::new(1.),
            enabled: Cell::new(true),
            material: Rc::new(self.material.camera_clip_variant()),
        });
        if clip.center != center || clip.distance != distance {
            return false;
        }
        clip.enabled.set(true);
        true
    }

    fn update_vertices(&self, updater: &dyn Fn(RefMut<VertexBuffer>)) {
        updater(self.vertices.borrow_mut());
        self.vertex_revision
            .set(self.vertex_revision.get().wrapping_add(1));
        let (aabb, centroid) = Self::compute_bounds(&self.vertices.borrow());
        self.local_aabb.set(aabb);
        self.local_centroid.set(centroid);
    }

    fn local_centroid(&self) -> [f32; 3] {
        self.local_centroid.get()
    }

    fn local_aabb(&self) -> Option<([f32; 3], [f32; 3])> {
        self.local_aabb.get()
    }

    fn set_uv_xform(&self, scale: [f32; 2], offset: [f32; 2]) {
        self.material.update_uv_xform(scale, offset);
    }

    fn material_debug_name(&self) -> Option<&str> {
        Some(self.material.debug_name())
    }

    fn material_texture_name(&self) -> Option<&str> {
        self.material.texture_names().first().map(String::as_str)
    }
}

impl VulkanRenderObject {
    pub fn new(
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material: Rc<VulkanMaterial>,
        host_dynamic: bool,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
        dub_manager: &Arc<DynamicUniformBufferManager>,
        descriptor_manager: &Rc<DescriptorManager>,
    ) -> Result<Self, Box<dyn Error>> {
        let vertex_buffer = if host_dynamic {
            None
        } else {
            Some(Buffer::new_device_buffer_with_data(
                allocator,
                BufferType::Vertex,
                vertices.data(),
                command_runner,
            )?)
        };

        let index_buffer = Buffer::new_device_buffer_with_data(
            allocator,
            BufferType::Index,
            &indices,
            command_runner,
        )?;

        let per_object_descriptor_set =
            descriptor_manager.allocate_per_object_descriptor_set(&material)?;
        let dub_index = dub_manager.allocate_buffer();
        let shadow_dub_index = dub_manager.allocate_buffer();

        let (local_aabb, local_centroid) = Self::compute_bounds(&vertices);

        Ok(Self {
            vertices: RefCell::new(vertices),
            _indices: indices,
            material,
            camera_clip: OnceCell::new(),
            host_dynamic,
            vertex_revision: Cell::new(0),
            dub_manager: dub_manager.clone(),
            vertex_buffer,
            index_buffer,
            per_object_descriptor_set,
            dub_index,
            shadow_dub_index,
            descriptor_manager: descriptor_manager.clone(),
            local_centroid: Cell::new(local_centroid),
            local_aabb: Cell::new(local_aabb),
        })
    }

    /// Local-space `(aabb, centroid)` of a vertex buffer's positions.
    fn compute_bounds(vertices: &VertexBuffer) -> (Option<([f32; 3], [f32; 3])>, [f32; 3]) {
        let aabb = vertices.aabb_min_max();
        let centroid = aabb
            .map(|(min, max)| {
                [
                    0.5 * (min[0] + max[0]),
                    0.5 * (min[1] + max[1]),
                    0.5 * (min[2] + max[2]),
                ]
            })
            .unwrap_or([0.0, 0.0, 0.0]);
        (aabb, centroid)
    }

    pub fn vertex_buffer(&self) -> Option<&Buffer> {
        self.vertex_buffer.as_ref()
    }

    pub fn vertices(&self) -> Ref<'_, VertexBuffer> {
        self.vertices.borrow()
    }

    pub fn has_dynamic_vertices(&self) -> bool {
        self.host_dynamic || self.vertex_revision.get() != 0
    }

    pub fn vertex_revision(&self) -> u64 {
        self.vertex_revision.get()
    }

    pub fn material_rc(&self) -> &Rc<VulkanMaterial> {
        if let Some(clip) = self
            .camera_clip
            .get()
            .filter(|clip| clip.opacity.get() < 1.)
        {
            &clip.material
        } else {
            &self.material
        }
    }

    pub(super) fn update_camera_clip(&self, world: &Mat44, camera: &Vec3) {
        let Some(clip) = self.camera_clip.get().filter(|clip| clip.enabled.get()) else {
            return;
        };
        let center = Vec3::new(
            world[0][0] * clip.center[0]
                + world[0][1] * clip.center[1]
                + world[0][2] * clip.center[2]
                + world[0][3],
            world[1][0] * clip.center[0]
                + world[1][1] * clip.center[1]
                + world[1][2] * clip.center[2]
                + world[1][3],
            world[2][0] * clip.center[0]
                + world[2][1] * clip.center[1]
                + world[2][2] * clip.center[2]
                + world[2][3],
        );
        let opacity = super::super::render_object::camera_clip_opacity(
            Vec3::sub(&center, camera).norm(),
            clip.distance,
        );
        clip.opacity.set(opacity);
        clip.material
            .update_camera_clip(self.material.params(), opacity);
    }

    pub(super) fn camera_clipped(&self) -> bool {
        self.camera_clip
            .get()
            .is_some_and(|clip| clip.opacity.get() == 0.)
    }

    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }

    pub fn dub_index(&self) -> usize {
        self.dub_index
    }

    pub fn shadow_dub_index(&self) -> usize {
        self.shadow_dub_index
    }

    pub fn material(&self) -> &VulkanMaterial {
        self.material_rc()
    }

    pub fn vk_descriptor_set(&self) -> vk::DescriptorSet {
        self.per_object_descriptor_set
    }
}

impl Drop for VulkanRenderObject {
    fn drop(&mut self) {
        self.descriptor_manager
            .free_per_object_descriptor_set(self.per_object_descriptor_set);
        self.dub_manager.deallocate_buffer(self.dub_index);
        self.dub_manager.deallocate_buffer(self.shadow_dub_index);
    }
}
