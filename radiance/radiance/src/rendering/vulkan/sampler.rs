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
    /// Create a Vulkan sampler matching the cross-backend `SamplerDef`,
    /// with a `max_lod` derived from the bound texture's mip count.
    ///
    /// Anisotropy is enabled (max 16) only when both filters are
    /// `Linear` — Vulkan validation requires `samplerAnisotropy` to be
    /// disabled when either filter is `NEAREST`. `min_lod` is pinned
    /// to 0; `max_lod` is `(mip_levels - 1) as f32` so single-level
    /// textures (`mip_levels = 1`) keep the legacy `max_lod = 0`
    /// behaviour and multi-level textures get the full chain.
    pub fn new(device: Rc<Device>, def: &SamplerDef, mip_levels: u32) -> VkResult<Self> {
        let anisotropy = def.uses_anisotropy();
        let mip_levels = mip_levels.max(1);
        let max_lod = (mip_levels - 1) as f32;
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
            .max_lod(max_lod);
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

/// Process-wide cache of `vk::Sampler` objects keyed by
/// `(SamplerDef, mip_levels)`.
///
/// Two materials that ask for the same RW sampler config and bind
/// textures with the same mip count share a single GPU sampler.
/// `max_lod` is derived from the mip count so a sampler for a
/// 1-level texture (`max_lod = 0`) is distinct from one for a
/// full mip chain (`max_lod > 0`).
pub struct VulkanSamplerCache {
    device: Rc<Device>,
    samplers: RefCell<HashMap<(SamplerDef, u32), Rc<Sampler>>>,
}

impl VulkanSamplerCache {
    pub fn new(device: Rc<Device>) -> Self {
        Self {
            device,
            samplers: RefCell::new(HashMap::new()),
        }
    }

    /// Return a sampler for `def` whose `max_lod` matches `mip_levels`.
    /// Callers that pass a texture-attached sampler should use the
    /// texture's `mip_levels()` here; samplers that don't bind to a
    /// mipmapped image (e.g. imgui, render-target previews) should
    /// pass `1`.
    pub fn get_or_create_for(&self, def: &SamplerDef, mip_levels: u32) -> Rc<Sampler> {
        let mip_levels = mip_levels.max(1);
        let key = (*def, mip_levels);
        if let Some(s) = self.samplers.borrow().get(&key) {
            return s.clone();
        }
        let sampler = Rc::new(
            Sampler::new(self.device.clone(), def, mip_levels)
                .expect("failed to create vk::Sampler from SamplerDef"),
        );
        self.samplers.borrow_mut().insert(key, sampler.clone());
        sampler
    }

    /// Convenience wrapper: get a sampler matching `def` for a
    /// non-mipmapped binding. Equivalent to
    /// `get_or_create_for(def, 1)`.
    pub fn get_or_create(&self, def: &SamplerDef) -> Rc<Sampler> {
        self.get_or_create_for(def, 1)
    }

    pub fn default_sampler(&self) -> Rc<Sampler> {
        self.get_or_create_for(&SamplerDef::DEFAULT, 1)
    }

    /// Sampler for UI / imgui textures (LINEAR + CLAMP_TO_EDGE). See
    /// [`SamplerDef::UI`]: CLAMP avoids the REPEAT edge-bleed that shows
    /// up as thin bright lines at sprite / 9-slice seams.
    pub fn ui_sampler(&self) -> Rc<Sampler> {
        self.get_or_create_for(&SamplerDef::UI, 1)
    }
}
