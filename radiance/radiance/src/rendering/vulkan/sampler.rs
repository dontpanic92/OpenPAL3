use super::device::Device;
use crate::rendering::{AddressMode, FilterMode, MipmapMode, SamplerDef};
use ash::prelude::VkResult;
use ash::vk;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Sampler {
    device: Rc<Device>,
    sampler: vk::Sampler,
}

fn vk_filter(f: FilterMode) -> vk::Filter {
    match f {
        FilterMode::Nearest => vk::Filter::NEAREST,
        FilterMode::Linear => vk::Filter::LINEAR,
    }
}

fn vk_mip_mode(m: MipmapMode) -> vk::SamplerMipmapMode {
    match m {
        MipmapMode::Nearest => vk::SamplerMipmapMode::NEAREST,
        MipmapMode::Linear => vk::SamplerMipmapMode::LINEAR,
    }
}

fn vk_address(a: AddressMode) -> vk::SamplerAddressMode {
    match a {
        AddressMode::Repeat => vk::SamplerAddressMode::REPEAT,
        AddressMode::Mirror => vk::SamplerAddressMode::MIRRORED_REPEAT,
        AddressMode::Clamp => vk::SamplerAddressMode::CLAMP_TO_EDGE,
        AddressMode::Border => vk::SamplerAddressMode::CLAMP_TO_BORDER,
    }
}

impl Sampler {
    /// Create a Vulkan sampler matching the cross-backend `SamplerDef`.
    ///
    /// Anisotropy is enabled (max 16) only when both filters are
    /// `Linear` — Vulkan validation requires `samplerAnisotropy` to be
    /// disabled when either filter is `NEAREST`. The mipmap LOD range
    /// is pinned to `[0, 0]` because `VulkanTexture` does not generate
    /// mip levels yet; the field is still threaded through so the
    /// chosen mipmap mode takes effect once mip generation lands.
    pub fn new(device: Rc<Device>, def: &SamplerDef) -> VkResult<Self> {
        let anisotropy = def.uses_anisotropy();
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk_filter(def.mag_filter))
            .min_filter(vk_filter(def.min_filter))
            .address_mode_u(vk_address(def.address_u))
            .address_mode_v(vk_address(def.address_v))
            .address_mode_w(vk_address(def.address_w))
            .anisotropy_enable(anisotropy)
            .max_anisotropy(if anisotropy { 16. } else { 1. })
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .mipmap_mode(vk_mip_mode(def.mipmap_mode))
            .mip_lod_bias(0.)
            .min_lod(0.)
            .max_lod(0.);
        let sampler = device.create_sampler(&sampler_info)?;
        Ok(Self {
            device: device.clone(),
            sampler,
        })
    }

    pub fn vk_sampler(&self) -> vk::Sampler {
        self.sampler
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.device.destroy_sampler(self.sampler);
    }
}

/// Process-wide cache of `vk::Sampler` objects keyed by `SamplerDef`.
///
/// Two materials that ask for the same RW sampler config (or the
/// shared default LINEAR/REPEAT used by every legacy call site) will
/// share a single GPU sampler. Lookups are O(1) per material binding;
/// the `RefCell` is fine because Vulkan resource creation always runs
/// on the rendering thread.
pub struct VulkanSamplerCache {
    device: Rc<Device>,
    samplers: RefCell<HashMap<SamplerDef, Rc<Sampler>>>,
}

impl VulkanSamplerCache {
    pub fn new(device: Rc<Device>) -> Self {
        Self {
            device,
            samplers: RefCell::new(HashMap::new()),
        }
    }

    pub fn get_or_create(&self, def: &SamplerDef) -> Rc<Sampler> {
        if let Some(s) = self.samplers.borrow().get(def) {
            return s.clone();
        }
        let sampler = Rc::new(
            Sampler::new(self.device.clone(), def)
                .expect("failed to create vk::Sampler from SamplerDef"),
        );
        self.samplers.borrow_mut().insert(*def, sampler.clone());
        sampler
    }

    pub fn default_sampler(&self) -> Rc<Sampler> {
        self.get_or_create(&SamplerDef::DEFAULT)
    }
}
