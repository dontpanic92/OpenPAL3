use imgui::TextureId;

use super::imgui::ImguiRenderer;
use super::texture::VulkanTextureStore;
use super::{
    adhoc_command_runner::AdhocCommandRunner, descriptor_managers::DescriptorManager,
    device::Device, material::VulkanMaterial, render_object::VulkanRenderObject,
    shader::VulkanShader, texture::VulkanTexture, uniform_buffers::DynamicUniformBufferManager,
};
use crate::rendering::VideoPlayer;
use crate::rendering::{
    factory::ComponentFactory, texture::TextureDef, Material, MaterialDef, RenderObject,
    RenderingComponent, Shader, ShaderDef, Texture, VertexBuffer,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

pub struct VulkanComponentFactory {
    device: Rc<Device>,
    allocator: Rc<vk_mem::Allocator>,
    descriptor_manager: Rc<DescriptorManager>,
    dub_manager: Arc<DynamicUniformBufferManager>,
    command_runner: Rc<AdhocCommandRunner>,
    texture_store: RefCell<VulkanTextureStore>,
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
        id: Option<TextureId>,
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
            .allocate_texture_descriptor_set(&texture)
            .unwrap();
        let texture_id = self.imgui.borrow_mut().upsert_texture(id, descriptor_set);
        self.descriptor_manager
            .free_texture_descriptor_set(descriptor_set);

        (Box::new(texture), texture_id)
    }

    fn create_shader(&self, shader_def: &ShaderDef) -> Box<dyn Shader> {
        Box::new(VulkanShader::new(shader_def, self.device.clone()).unwrap())
    }

    fn create_material(&self, material_def: &MaterialDef) -> Box<dyn Material> {
        let mut texture_store = self.texture_store.borrow_mut();
        Box::new(VulkanMaterial::new(
            material_def,
            &self.device,
            &self.allocator,
            &self.command_runner,
            &mut texture_store,
        ))
    }

    fn create_render_object(
        &self,
        vertices: VertexBuffer,
        indices: Vec<u32>,
        material_def: &MaterialDef,
        host_dynamic: bool,
    ) -> Box<dyn RenderObject> {
        let material = self.create_material(material_def);
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
        Self {
            device,
            allocator: allocator.clone(),
            descriptor_manager: descriptor_manager.clone(),
            dub_manager: dub_manager.clone(),
            command_runner: command_runner.clone(),
            texture_store: RefCell::new(VulkanTextureStore::new()),
            imgui,
        }
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
