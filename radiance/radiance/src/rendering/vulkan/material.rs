use super::descriptor_managers::DescriptorManager;
use super::sampler::Sampler;
use super::shader_cache::VulkanShaderCache;
use super::texture::VulkanTextureStore;
use super::{device::Device, shader::VulkanShader, texture::VulkanTexture};
use crate::rendering::vulkan::adhoc_command_runner::AdhocCommandRunner;
use crate::rendering::{BlendMode, DepthMode, MaterialDef, MaterialKey, MaterialParams};
use std::cell::Cell;
use std::rc::Rc;

pub struct VulkanMaterial {
    debug_name: String,
    key: MaterialKey,
    shader: Rc<VulkanShader>,
    textures: Vec<Rc<VulkanTexture>>,
    texture_names: Vec<String>,
    samplers: Vec<Rc<Sampler>>,
    /// Current per-material parameters. The `tint` / `misc` fields are
    /// frozen at construction; `uv_scale` / `uv_offset` may be updated at
    /// runtime via [`update_uv_xform`] to drive UV animation (PAL4
    /// water).
    params: Cell<MaterialParams>,

    revision: Cell<u64>,
}

impl std::fmt::Debug for VulkanMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("VulkanMaterial: {}", &self.debug_name))
    }
}

impl VulkanMaterial {
    pub(super) fn camera_clip_variant(&self) -> Self {
        let mut key = self.key;
        key.blend = BlendMode::AlphaBlend;
        key.depth = DepthMode::TestOnly;
        Self {
            debug_name: self.debug_name.clone(),
            key,
            shader: self.shader.clone(),
            textures: self.textures.clone(),
            texture_names: self.texture_names.clone(),
            samplers: self.samplers.clone(),
            params: Cell::new(self.params()),
            revision: Cell::new(0),
        }
    }

    pub(super) fn update_camera_clip(&self, mut params: MaterialParams, opacity: f32) {
        params.tint[3] *= opacity;
        if params != self.params.get() {
            self.params.set(params);
            self.revision.set(self.revision.get().wrapping_add(1));
        }
    }

    pub fn new(
        def: &MaterialDef,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
        texture_store: &mut VulkanTextureStore,
        shader_cache: &VulkanShaderCache,
        descriptor_manager: &Rc<DescriptorManager>,
    ) -> Self {
        let _ = device;
        let shader = shader_cache.get_or_create(def.program());
        let texture_names: Vec<String> = def
            .textures()
            .iter()
            .map(|t| t.name().to_string())
            .collect();
        let textures: Vec<Rc<VulkanTexture>> = def
            .textures()
            .iter()
            .map(|t| {
                texture_store.get_or_update(t.cache_key(), || {
                    VulkanTexture::new(t, device, allocator, command_runner).unwrap()
                })
            })
            .collect();
        // Pair each sampler with its bound texture's mip count so
        // `max_lod = mip_levels - 1` covers the chain generated in
        // `VulkanTexture::from_buffer`. Falls back to `mip_levels = 1`
        // when textures/samplers are mismatched in length (legacy
        // callers occasionally set fewer samplers than textures).
        let samplers = def
            .samplers()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let mips = textures.get(i).map(|t| t.mip_levels()).unwrap_or(1);
                descriptor_manager
                    .sampler_cache()
                    .get_or_create_for(s, mips)
            })
            .collect();

        Self {
            debug_name: def.debug_name().to_string(),
            key: def.key(),
            shader,
            textures,
            texture_names,
            samplers,
            params: Cell::new(*def.params()),
            revision: Cell::new(0),
        }
    }

    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    pub fn key(&self) -> &MaterialKey {
        &self.key
    }

    pub fn shader(&self) -> &VulkanShader {
        &self.shader
    }

    pub fn textures(&self) -> &[Rc<VulkanTexture>] {
        &self.textures
    }

    /// Per-binding texture names captured from the source `MaterialDef`
    /// at construction time. Exposed so callers (e.g. the PAL4
    /// `UvAnimationComponent` inventory dump) can answer "what texture does
    /// material X use?" without needing the original `MaterialDef`.
    pub fn texture_names(&self) -> &[String] {
        &self.texture_names
    }

    pub fn samplers(&self) -> &[Rc<Sampler>] {
        &self.samplers
    }

    pub fn params(&self) -> MaterialParams {
        self.params.get()
    }

    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Stage a new UV affine for the next frame-owned material snapshot.
    /// (`scale = xy`, `offset = zw`). Used by the runtime
    /// `UvAnimComponent` to drive PAL4 water animation each tick.
    ///
    /// `tint` and `misc` are preserved from the immutable `MaterialDef`
    /// captured at construction; only the `uv_xform` slot is rewritten.
    /// Uses interior mutability so the existing `Rc<VulkanMaterial>`
    /// sharing pattern keeps working.
    pub fn update_uv_xform(&self, scale: [f32; 2], offset: [f32; 2]) {
        let mut p = self.params.get();
        p.uv_scale = scale;
        p.uv_offset = offset;
        self.params.set(p);
        self.revision.set(self.revision.get().wrapping_add(1));
    }
}
