use imgui::TextureId;

use super::imgui::ImguiRenderer;
use super::shader_cache::VulkanShaderCache;
use super::texture::VulkanTextureStore;
use super::{
    adhoc_command_runner::AdhocCommandRunner, descriptor_managers::DescriptorManager,
    device::Device, material::VulkanMaterial, render_object::VulkanRenderObject,
    texture::VulkanTexture, uniform_buffers::DynamicUniformBufferManager,
};
use crate::rendering::VideoPlayer;
use crate::rendering::{
    factory::ComponentFactory, texture::TextureDef, MaterialDef, MaterialKey, MaterialParams,
    RenderObject, RenderingComponent, Texture, VertexBuffer,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Bit-pattern view of [`MaterialParams`] so it can be a `HashMap` key.
/// Two materials with identical floats hash equal; `+0.0`/`-0.0` and NaN
/// patterns are *not* normalized (this is the desired behavior — we only
/// care about exact equality).
#[derive(Hash, Eq, PartialEq, Clone, Copy)]
struct MaterialParamsBits {
    tint: [u32; 4],
    alpha_ref: u32,
    uv_scale: [u32; 2],
    uv_offset: [u32; 2],
}

impl From<&MaterialParams> for MaterialParamsBits {
    fn from(p: &MaterialParams) -> Self {
        Self {
            tint: [
                p.tint[0].to_bits(),
                p.tint[1].to_bits(),
                p.tint[2].to_bits(),
                p.tint[3].to_bits(),
            ],
            alpha_ref: p.alpha_ref.to_bits(),
            uv_scale: [p.uv_scale[0].to_bits(), p.uv_scale[1].to_bits()],
            uv_offset: [p.uv_offset[0].to_bits(), p.uv_offset[1].to_bits()],
        }
    }
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct MaterialIdentity {
    key: MaterialKey,
    texture_names: Vec<String>,
    params: MaterialParamsBits,
}

impl MaterialIdentity {
    fn from(def: &MaterialDef) -> Self {
        Self {
            key: def.key(),
            texture_names: def.textures().iter().map(|t| t.name().to_string()).collect(),
            params: MaterialParamsBits::from(def.params()),
        }
    }
}

pub struct VulkanComponentFactory {
    device: Rc<Device>,
    allocator: Rc<vk_mem::Allocator>,
    descriptor_manager: Rc<DescriptorManager>,
    dub_manager: Arc<DynamicUniformBufferManager>,
    command_runner: Rc<AdhocCommandRunner>,
    texture_store: RefCell<VulkanTextureStore>,
    shader_cache: Rc<VulkanShaderCache>,
    material_cache: RefCell<HashMap<MaterialIdentity, Rc<VulkanMaterial>>>,
    imgui: Rc<RefCell<ImguiRenderer>>,
}

impl ComponentFactory for VulkanComponentFactory {
    fn create_texture(&self, texture_def: &TextureDef) -> Box<dyn Texture> {
        Box::new(
            VulkanTexture::new(
                texture_def,
                &self.device,
                &self.allocator,
                &self.command_runner,
            )
            .unwrap(),
        )
    }

    fn create_imgui_texture(
        &self,
        buffer: &[u8],
        row_length: u32,
        width: u32,
        height: u32,
        texture_id: Option<TextureId>,
    ) -> (Box<dyn Texture>, TextureId) {
        let texture = VulkanTexture::from_buffer(
            buffer,
            row_length,
            width,
            height,
            &self.device,
            &self.allocator,
            &self.command_runner,
        )
        .unwrap();

        let descriptor_set = self
            .descriptor_manager
            .create_texture_descriptor_set(&texture);

        let texture_id = self
            .imgui
            .borrow_mut()
            .upsert_texture(texture_id, descriptor_set);
        (Box::new(texture), texture_id)
    }

    fn remove_imgui_texture(&self, texture_id: Option<TextureId>) {
        self.imgui.borrow_mut().remove_texture(texture_id);
    }

    fn create_render_object(
        &self,
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material_def: &MaterialDef,
        host_dynamic: bool,
    ) -> Box<dyn RenderObject> {
        let material = self.get_or_create_material(material_def);
        let x = Box::new(
            VulkanRenderObject::new(
                vertices,
                indices,
                material,
                host_dynamic,
                &self.allocator,
                &self.command_runner,
                &self.dub_manager,
                &self.descriptor_manager,
            )
            .unwrap(),
        );
        x
    }

    fn create_rendering_component(
        &self,
        objects: Vec<Box<dyn RenderObject>>,
    ) -> RenderingComponent {
        let mut component = RenderingComponent::new();
        for o in objects {
            component.push_render_object(o);
        }

        component
    }

    fn create_video_player(&self) -> Box<VideoPlayer> {
        Box::new(VideoPlayer::new())
    }
}

impl VulkanComponentFactory {
    pub fn new(
        device: Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        descriptor_manager: &Rc<DescriptorManager>,
        dub_manager: &Arc<DynamicUniformBufferManager>,
        command_runner: &Rc<AdhocCommandRunner>,
        imgui: Rc<RefCell<ImguiRenderer>>,
    ) -> Self {
        let shader_cache = Rc::new(VulkanShaderCache::new(device.clone()));
        Self {
            device,
            allocator: allocator.clone(),
            descriptor_manager: descriptor_manager.clone(),
            dub_manager: dub_manager.clone(),
            command_runner: command_runner.clone(),
            texture_store: RefCell::new(VulkanTextureStore::new()),
            shader_cache,
            material_cache: RefCell::new(HashMap::new()),
            imgui,
        }
    }

    fn get_or_create_material(&self, def: &MaterialDef) -> Rc<VulkanMaterial> {
        let identity = MaterialIdentity::from(def);
        if let Some(existing) = self.material_cache.borrow().get(&identity) {
            return existing.clone();
        }

        let material = {
            let mut texture_store = self.texture_store.borrow_mut();
            Rc::new(VulkanMaterial::new(
                def,
                &self.device,
                &self.allocator,
                &self.command_runner,
                &mut texture_store,
                &self.shader_cache,
                &self.descriptor_manager,
            ))
        };
        self.material_cache
            .borrow_mut()
            .insert(identity, material.clone());
        material
    }

    pub fn as_component_factory(self: &Rc<Self>) -> Rc<dyn ComponentFactory> {
        self.clone()
    }

    pub fn _create_vulkan_texture(&self, texture_def: &TextureDef) -> Rc<VulkanTexture> {
        self.texture_store
            .borrow_mut()
            .get_or_update(texture_def.name(), || {
                VulkanTexture::new(
                    texture_def,
                    &self.device,
                    &self.allocator,
                    &self.command_runner,
                )
                .unwrap()
            })
    }
}
