use super::buffer::{Buffer, BufferType};
use super::descriptor_managers::DescriptorManager;
use super::descriptor_pool::DescriptorPool;
use super::material::VulkanMaterial;
use super::render_object::VulkanRenderObject;
use super::uniform_buffers::{
    DynamicUniformBufferManager, MaterialParamsGpu, PerInstanceUniformBuffer,
};
use crate::comdef::{IEntity, IEntityExt};
use crate::math::Mat44;
use ash::vk;
use crosscom::ComRc;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

struct FrameObject {
    object: Rc<VulkanRenderObject>,
    vertices: Option<Buffer>,
    revision: u64,
}

struct FrameMaterial {
    _material: Rc<VulkanMaterial>,
    buffer: Buffer,
    descriptor_set: vk::DescriptorSet,
    revision: u64,
    descriptor_manager: Rc<DescriptorManager>,
    descriptor_pool: Rc<DescriptorPool>,
}

impl Drop for FrameMaterial {
    fn drop(&mut self) {
        self.descriptor_manager
            .free_material_params_descriptor_set(&self.descriptor_pool, self.descriptor_set);
    }
}

/// GPU snapshots and resource references owned by one submission slot.
/// Prepare or retire this slot only after its submission fence signals.
pub(super) struct DrawFrame {
    // Drop GPU resources before the allocator/device-owning managers.
    models: Buffer,
    objects: HashMap<usize, FrameObject>,
    materials: HashMap<usize, FrameMaterial>,
    allocator: Rc<vk_mem::Allocator>,
    descriptor_manager: Rc<DescriptorManager>,
    material_pool: Rc<DescriptorPool>,
    descriptor_set: vk::DescriptorSet,
    alignment: u64,
    used_objects: HashSet<usize>,
    used_materials: HashSet<usize>,
    matrices: Vec<(usize, Mat44)>,
}

impl DrawFrame {
    pub fn new(
        allocator: &Rc<vk_mem::Allocator>,
        descriptor_manager: &Rc<DescriptorManager>,
        manager: &DynamicUniformBufferManager,
    ) -> Self {
        let models = Buffer::new_dynamic_buffer(
            allocator,
            BufferType::Uniform,
            manager.alignment() as usize,
            64,
        )
        .expect("allocate frame model buffer");
        let descriptor_set = descriptor_manager
            .dub_descriptor_manager()
            .allocate_descriptor_sets(&models);
        Self {
            allocator: allocator.clone(),
            descriptor_manager: descriptor_manager.clone(),
            material_pool: descriptor_manager.create_frame_material_pool(),
            models,
            descriptor_set,
            alignment: manager.alignment(),
            objects: HashMap::new(),
            materials: HashMap::new(),
            used_objects: HashSet::new(),
            used_materials: HashSet::new(),
            matrices: Vec::new(),
        }
    }

    pub fn prepare(&mut self, entities: &[ComRc<IEntity>]) -> u64 {
        self.used_objects.clear();
        self.used_materials.clear();
        self.matrices.clear();
        let mut count = 0;
        for entity in entities {
            if let Some(component) = entity.get_rendering_component() {
                let world = entity.world_transform();
                let shadow = if entity
                    .get_component(crate::comdef::IBillboardComponent::uuid())
                    .is_some()
                {
                    crate::components::billboard::billboard_shadow_matrix(
                        world.matrix(),
                        entity.transform().borrow().matrix(),
                    )
                } else {
                    *world.matrix()
                };
                for object in component.vulkan_render_objects() {
                    self.prepare_object(object);
                    self.matrices.push((object.dub_index(), *world.matrix()));
                    self.matrices.push((object.shadow_dub_index(), shadow));
                    count += 1;
                }
            }
        }
        self.objects
            .retain(|key, _| self.used_objects.contains(key));
        self.materials
            .retain(|key, _| self.used_materials.contains(key));
        let required = self
            .matrices
            .iter()
            .map(|(id, _)| id + 1)
            .max()
            .unwrap_or(0);
        self.ensure_model_capacity(required);
        if self.matrices.is_empty() {
            return count;
        }
        let alignment = self.alignment;
        self.models.map_memory_do(|dst| {
            for (id, matrix) in &self.matrices {
                unsafe {
                    (dst.add(*id * alignment as usize) as *mut PerInstanceUniformBuffer)
                        .write(PerInstanceUniformBuffer::_new(matrix));
                }
            }
        });
        count
    }

    fn ensure_model_capacity(&mut self, required: usize) {
        if required <= self.models.element_count() as usize {
            return;
        }
        let models = Buffer::new_dynamic_buffer(
            &self.allocator,
            BufferType::Uniform,
            self.alignment as usize,
            required.next_power_of_two(),
        )
        .expect("grow frame model buffer");
        let descriptors = self.descriptor_manager.dub_descriptor_manager();
        let descriptor_set = descriptors.allocate_descriptor_sets(&models);
        descriptors.free_descriptor_set(self.descriptor_set);
        self.descriptor_set = descriptor_set;
        self.models = models;
    }

    fn prepare_object(&mut self, object: &Rc<VulkanRenderObject>) {
        let key = Rc::as_ptr(object) as usize;
        self.used_objects.insert(key);
        let revision = object.vertex_revision();
        let entry = self.objects.entry(key).or_insert_with(|| FrameObject {
            object: object.clone(),
            vertices: None,
            revision: 0,
        });
        if object.has_dynamic_vertices() {
            if let Some(buffer) = &mut entry.vertices {
                if entry.revision != revision {
                    buffer.copy_memory_from(object.vertices().data());
                }
            } else {
                entry.vertices = Some(
                    Buffer::new_dynamic_buffer_with_data(
                        &self.allocator,
                        BufferType::Vertex,
                        object.vertices().data(),
                    )
                    .expect("allocate frame vertex buffer"),
                );
            }
            entry.revision = revision;
        }
        let material = object.material_rc();
        let key = Rc::as_ptr(material) as usize;
        self.used_materials.insert(key);
        let revision = material.revision();
        let entry = self.materials.entry(key).or_insert_with(|| {
            let buffer = Buffer::new_dynamic_buffer_with_data(
                &self.allocator,
                BufferType::Uniform,
                &[MaterialParamsGpu::from_params(&material.params())],
            )
            .expect("allocate frame material buffer");
            let descriptor_set = self
                .descriptor_manager
                .allocate_material_params_descriptor_set(&self.material_pool, &buffer)
                .expect("allocate frame material descriptor");
            FrameMaterial {
                _material: material.clone(),
                buffer,
                descriptor_set,
                revision,
                descriptor_manager: self.descriptor_manager.clone(),
                descriptor_pool: self.material_pool.clone(),
            }
        });
        if entry.revision != revision {
            entry
                .buffer
                .copy_memory_from(&[MaterialParamsGpu::from_params(&material.params())]);
            entry.revision = revision;
        }
    }

    pub fn vertex_buffer(&self, object: &VulkanRenderObject) -> vk::Buffer {
        let entry = &self.objects[&(object as *const _ as usize)];
        entry
            .vertices
            .as_ref()
            .or_else(|| entry.object.vertex_buffer())
            .unwrap()
            .vk_buffer()
    }

    pub fn material_descriptor_set(&self, material: &VulkanMaterial) -> vk::DescriptorSet {
        self.materials[&(material as *const _ as usize)].descriptor_set
    }

    pub fn descriptor_set(&self) -> vk::DescriptorSet {
        self.descriptor_set
    }

    pub fn get_offset(&self, id: usize) -> u64 {
        self.alignment * id as u64
    }
}

impl Drop for DrawFrame {
    fn drop(&mut self) {
        self.descriptor_manager
            .dub_descriptor_manager()
            .free_descriptor_set(self.descriptor_set);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        adhoc_command_runner::AdhocCommandRunner, device::Device, instance::Instance,
        shader_cache::VulkanShaderCache, texture::VulkanTextureStore,
    };
    use super::*;
    use crate::math::Transform;
    use crate::rendering::{
        MaterialDef, RenderObject, RenderObjectHandle, RenderingComponent, SamplerDef,
        ShaderProgram, TextureStore, VertexBuffer, VertexComponents,
    };
    use std::sync::Arc;

    fn model_snapshot(frame: &mut DrawFrame, id: usize) -> Vec<u8> {
        let offset = frame.get_offset(id) as usize;
        let end = offset + std::mem::size_of::<PerInstanceUniformBuffer>();
        frame.models.read_into_vec(end)[offset..end].to_vec()
    }

    #[test]
    #[ignore = "requires a working Vulkan loader and ICD"]
    fn frame_snapshots_isolate_vertices_models_materials_and_keep_draws_alive() {
        let entry = Rc::new(unsafe { ash::Entry::load().unwrap() });
        let instance = Rc::new(Instance::new(entry));
        let physical_device =
            super::super::creation_helpers::get_physical_device(instance.vk_instance()).unwrap();
        let queue_family = unsafe {
            instance
                .vk_instance()
                .get_physical_device_queue_family_properties(physical_device)
        }
        .iter()
        .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        .unwrap() as u32;
        let device = Rc::new(Device::new(instance.clone(), physical_device, queue_family));
        let allocator = Rc::new(unsafe {
            vk_mem::Allocator::new(vk_mem::AllocatorCreateInfo::new(
                instance.vk_instance(),
                device.vk_device(),
                physical_device,
            ))
            .unwrap()
        });
        let command_pool = device
            .create_command_pool(
                &vk::CommandPoolCreateInfo::default().queue_family_index(queue_family),
            )
            .unwrap();
        let runner = Rc::new(AdhocCommandRunner::new(
            device.clone(),
            command_pool,
            device.get_device_queue(queue_family, 0),
        ));
        {
            let descriptors = Rc::new(
                DescriptorManager::new(
                    device.clone(),
                    &instance,
                    physical_device,
                    &allocator,
                    &runner,
                )
                .unwrap(),
            );
            let alignment = instance
                .get_physical_device_properties(physical_device)
                .limits
                .min_uniform_buffer_offset_alignment;
            let manager = Arc::new(DynamicUniformBufferManager::new(alignment));
            let reserved = (0..80)
                .map(|_| manager.allocate_buffer())
                .collect::<Vec<_>>();
            let mut frames = [
                DrawFrame::new(&allocator, &descriptors, &manager),
                DrawFrame::new(&allocator, &descriptors, &manager),
            ];
            let texture =
                TextureStore::get_or_update_opaque("test::frame-snapshot-texture", || {
                    Some(image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4])))
                });
            let def = MaterialDef::builder(ShaderProgram::Pal3Prop)
                .textures_with_samplers(vec![texture], vec![SamplerDef::default()])
                .build();
            let shaders = VulkanShaderCache::new(device.clone());
            let material = Rc::new(VulkanMaterial::new(
                &def,
                &device,
                &allocator,
                &runner,
                &mut VulkanTextureStore::new(),
                &shaders,
                &descriptors,
            ));
            let object = Rc::new(
                VulkanRenderObject::new(
                    VertexBuffer::new(VertexComponents::POSITION, 1),
                    vec![0],
                    material.clone(),
                    true,
                    &allocator,
                    &runner,
                    &manager,
                    &descriptors,
                )
                .unwrap(),
            );
            let weak = Rc::downgrade(&object);
            let entity = crate::scene::CoreEntity::create("frame snapshot".into(), true);
            let mut component = RenderingComponent::new();
            component.push_render_object(RenderObjectHandle::from_vulkan(object.clone()));
            entity.set_rendering_component(Some(Rc::new(component)));
            frames[0].prepare(&[entity.clone()]);
            let object_key = Rc::as_ptr(&object) as usize;
            let material_key = Rc::as_ptr(&material) as usize;
            let old_vertices = frames[0]
                .objects
                .get_mut(&object_key)
                .unwrap()
                .vertices
                .as_mut()
                .unwrap()
                .read_into_vec(12);
            assert!(
                frames[0].models.element_count() as usize > object.shadow_dub_index(),
                "grow before writing high model IDs"
            );
            let old_models = model_snapshot(&mut frames[0], object.dub_index());
            let old_shadow = model_snapshot(&mut frames[0], object.shadow_dub_index());
            let old_material = frames[0]
                .materials
                .get_mut(&material_key)
                .unwrap()
                .buffer
                .read_into_vec(std::mem::size_of::<MaterialParamsGpu>());
            object.update_vertices(&|mut vertices| {
                vertices
                    .set_vertex_blob(0, |bytes| bytes[0..4].copy_from_slice(&7f32.to_ne_bytes()))
            });
            material.update_uv_xform([2., 3.], [0.25, 0.5]);
            let mut matrix = Mat44::new_identity();
            matrix[0][3] = 5.;
            let mut transform = Transform::new();
            transform.set_matrix(matrix);
            entity.update_world_transform(&transform);
            frames[1].prepare(&[entity.clone()]);
            assert_eq!(
                frames[0]
                    .objects
                    .get_mut(&object_key)
                    .unwrap()
                    .vertices
                    .as_mut()
                    .unwrap()
                    .read_into_vec(12),
                old_vertices
            );
            assert_eq!(
                model_snapshot(&mut frames[0], object.dub_index()),
                old_models
            );
            assert_eq!(
                model_snapshot(&mut frames[0], object.shadow_dub_index()),
                old_shadow
            );
            assert_eq!(
                frames[0]
                    .materials
                    .get_mut(&material_key)
                    .unwrap()
                    .buffer
                    .read_into_vec(old_material.len()),
                old_material
            );
            assert_ne!(
                frames[1]
                    .objects
                    .get_mut(&object_key)
                    .unwrap()
                    .vertices
                    .as_mut()
                    .unwrap()
                    .read_into_vec(12),
                old_vertices
            );
            assert_ne!(
                model_snapshot(&mut frames[1], object.dub_index()),
                old_models
            );
            assert_ne!(
                model_snapshot(&mut frames[1], object.shadow_dub_index()),
                old_shadow
            );
            assert_ne!(
                frames[1]
                    .materials
                    .get_mut(&material_key)
                    .unwrap()
                    .buffer
                    .read_into_vec(old_material.len()),
                old_material
            );
            let vertex_handle = frames[0].vertex_buffer(&object);
            frames[0].prepare(&[entity.clone()]);
            assert_eq!(
                frames[0].vertex_buffer(&object),
                vertex_handle,
                "reuse allocation when updating a retired frame"
            );
            assert_eq!(
                frames[0]
                    .objects
                    .get_mut(&object_key)
                    .unwrap()
                    .vertices
                    .as_mut()
                    .unwrap()
                    .read_into_vec(12),
                frames[1]
                    .objects
                    .get_mut(&object_key)
                    .unwrap()
                    .vertices
                    .as_mut()
                    .unwrap()
                    .read_into_vec(12)
            );
            let clip_def = MaterialDef::builder(ShaderProgram::TexturedNoLight).build();
            let clip_material = Rc::new(VulkanMaterial::new(
                &clip_def,
                &device,
                &allocator,
                &runner,
                &mut VulkanTextureStore::new(),
                &shaders,
                &descriptors,
            ));
            let make_object = || {
                Rc::new(
                    VulkanRenderObject::new(
                        VertexBuffer::new(VertexComponents::POSITION, 1),
                        vec![0],
                        clip_material.clone(),
                        true,
                        &allocator,
                        &runner,
                        &manager,
                        &descriptors,
                    )
                    .unwrap(),
                )
            };
            let clipped = make_object();
            let sibling = make_object();
            assert!(clipped.set_camera_clip(Some(([10., 20., 30.], 1200.))));
            let base_params = clip_material.params();
            let mut world = Transform::new();
            world
                .scale_local(&crate::math::Vec3::new(2., 3., 4.))
                .rotate_axis_angle(&crate::math::Vec3::UP, std::f32::consts::FRAC_PI_2)
                .set_position(&crate::math::Vec3::new(100., 200., 300.));
            let center = crate::math::Vec3::new(220., 260., 280.);
            let camera_at =
                |distance| crate::math::Vec3::new(center.x, center.y + distance, center.z);
            clipped.update_camera_clip(world.matrix(), &camera_at(1225.));
            assert!(!clipped.camera_clipped());
            assert_eq!(
                clipped.material().key().blend,
                crate::rendering::BlendMode::AlphaBlend
            );
            assert_eq!(
                clipped.material().key().depth,
                crate::rendering::DepthMode::TestOnly
            );
            assert!(
                (clipped.material().params().tint[3] - base_params.tint[3] * 128. / 255.).abs()
                    < 0.0001
            );
            assert_eq!(sibling.material().params(), base_params);
            assert!(Rc::ptr_eq(sibling.material_rc(), &clip_material));
            let clip_entity = crate::scene::CoreEntity::create("camera clip snapshot".into(), true);
            let mut component = RenderingComponent::new();
            component.push_render_object(RenderObjectHandle::from_vulkan(clipped.clone()));
            clip_entity.set_rendering_component(Some(Rc::new(component)));
            frames[0].prepare(&[clip_entity.clone()]);
            let faded_key = Rc::as_ptr(clipped.material_rc()) as usize;
            let faded_snapshot = frames[0]
                .materials
                .get_mut(&faded_key)
                .unwrap()
                .buffer
                .read_into_vec(std::mem::size_of::<MaterialParamsGpu>());
            clipped.update_camera_clip(world.matrix(), &camera_at(1200.));
            assert!(clipped.camera_clipped());
            frames[1].prepare(&[clip_entity.clone()]);
            assert_eq!(
                frames[0]
                    .materials
                    .get_mut(&faded_key)
                    .unwrap()
                    .buffer
                    .read_into_vec(faded_snapshot.len()),
                faded_snapshot
            );
            clipped.update_camera_clip(world.matrix(), &camera_at(1250.));
            assert!(!clipped.camera_clipped());
            assert!(Rc::ptr_eq(clipped.material_rc(), &clip_material));
            assert_eq!(clipped.material().key(), clip_material.key());
            assert_eq!(clip_material.params(), base_params);
            clipped.update_camera_clip(world.matrix(), &camera_at(1199.));
            assert!(clipped.camera_clipped());
            assert!(clipped.set_camera_clip(None));
            assert!(!clipped.camera_clipped());
            assert!(Rc::ptr_eq(clipped.material_rc(), &clip_material));
            frames[1].prepare(&[entity.clone()]);
            drop(entity);
            drop(object);
            frames[0].prepare(&[]);
            assert!(
                weak.upgrade().is_some(),
                "other submission still owns the draw"
            );
            frames[1].prepare(&[]);
            assert!(
                weak.upgrade().is_none(),
                "retired submissions release the draw"
            );
            for id in reserved {
                manager.deallocate_buffer(id);
            }
        }
        device.wait_idle();
        device.destroy_command_pool(command_pool);
    }
}
