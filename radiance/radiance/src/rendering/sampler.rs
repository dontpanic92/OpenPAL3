//! Cross-backend texture-sampler description.
//!
//! `SamplerDef` describes how a texture should be sampled by the shader
//! — independent of any particular GPU backend. Loaders that know about
//! source-asset sampler state (e.g. RenderWare's
//! `Texture::filter_mode/address_mode_u/address_mode_v`) construct a
//! `SamplerDef` and attach it to a per-binding slot on the
//! `MaterialDef`. The Vulkan backend then translates the enum values
//! into `vk::Filter` / `vk::SamplerAddressMode` / `vk::SamplerMipmapMode`
//! and looks up an `Rc<Sampler>` from a sampler cache keyed by
//! `SamplerDef`, so two materials that ask for the same sampler config
//! share the GPU object.
//!
//! The `Default` impl reproduces today's hardcoded behavior
//! (LINEAR + REPEAT in all axes), so any caller that doesn't supply a
//! `SamplerDef` keeps the legacy behavior bit-for-bit.
//!
//! Mipmap support is intentionally minimal: the field is carried but
//! the Vulkan texture pipeline does not generate mip levels yet, so
//! the chosen `MipmapMode` only takes effect once mip generation is
//! wired up. `From<rwbs filter_mode>` collapses RW's MIP_* values to
//! the matching non-mipped filter to avoid wrongly sampling a
//! single-level image with a mip-aware sampler.

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum FilterMode {
    Nearest,
    Linear,
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum MipmapMode {
    Nearest,
    Linear,
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum AddressMode {
    Repeat,
    Mirror,
    Clamp,
    Border,
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct SamplerDef {
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_mode: MipmapMode,
    pub address_u: AddressMode,
    pub address_v: AddressMode,
    pub address_w: AddressMode,
}

impl SamplerDef {
    pub const DEFAULT: SamplerDef = SamplerDef {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_mode: MipmapMode::Linear,
        address_u: AddressMode::Repeat,
        address_v: AddressMode::Repeat,
        address_w: AddressMode::Repeat,
    };

    /// Sampler for UI / imgui textures (sprites, 9-slice chrome, video,
    /// render-target previews): LINEAR filtering with CLAMP_TO_EDGE on
    /// every axis. UI sprites are drawn edge-to-edge with `[0,1]` UVs, so
    /// REPEAT addressing makes the bilinear sampler wrap at the `u=0/1` /
    /// `v=0/1` border and bleed the opposite-edge texel — appearing as
    /// thin bright lines at 9-slice seams. CLAMP_TO_EDGE pins the edge
    /// texel and removes that bleed.
    pub const UI: SamplerDef = SamplerDef::new(FilterMode::Linear, AddressMode::Clamp);

    pub const fn new(filter: FilterMode, address: AddressMode) -> Self {
        Self {
            mag_filter: filter,
            min_filter: filter,
            mipmap_mode: MipmapMode::Linear,
            address_u: address,
            address_v: address,
            address_w: address,
        }
    }

    pub const fn with_address_uv(
        filter: FilterMode,
        address_u: AddressMode,
        address_v: AddressMode,
    ) -> Self {
        Self {
            mag_filter: filter,
            min_filter: filter,
            mipmap_mode: MipmapMode::Linear,
            address_u,
            address_v,
            address_w: AddressMode::Repeat,
        }
    }

    pub fn uses_anisotropy(&self) -> bool {
        // Per Vulkan spec validation, samplerAnisotropy must be disabled
        // when either mag or min filter is NEAREST. Pixel-art-style
        // assets that explicitly request NEAREST also look better
        // without anisotropic filtering.
        matches!(self.mag_filter, FilterMode::Linear)
            && matches!(self.min_filter, FilterMode::Linear)
    }
}

impl Default for SamplerDef {
    fn default() -> Self {
        Self::DEFAULT
    }
}
