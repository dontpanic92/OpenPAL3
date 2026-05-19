use super::{
    adhoc_command_runner::AdhocCommandRunner, buffer::Buffer, device::Device, image::Image,
    image_view::ImageView,
};
use crate::rendering::texture::{Texture, TextureDef};
use ash::vk;
use lru::LruCache;
use std::error::Error;
use std::num::NonZero;
use std::rc::Rc;

pub struct VulkanTexture {
    image: Image,
    image_view: ImageView,
}

impl Texture for VulkanTexture {
    fn width(&self) -> u32 {
        self.image.width()
    }

    fn height(&self) -> u32 {
        self.image.height()
    }
}

impl VulkanTexture {
    pub fn new(
        def: &TextureDef,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
    ) -> Result<Self, Box<dyn Error>> {
        // Drain the CPU-side `RgbaImage` out of the `TextureDef` here:
        // after this upload the GPU copy is the source of truth and the
        // CPU bytes are dead weight that would otherwise linger in
        // `TEXTURE_STORE` for the process lifetime. `VulkanTextureStore`
        // caches `Rc<VulkanTexture>` by texture name so a repeat
        // `create_texture` call for the same `TextureDef` short-circuits
        // before reaching here — `take_image` returning `None` on a
        // re-entry path only happens if the same `TextureDef` is wired
        // through two different `VulkanTextureStore` instances, in
        // which case falling back to the missing-texture sentinel keeps
        // rendering correct (just visually wrong for that one texture).
        let owned_image = def.take_image();
        let texture_missing =
            image::load_from_memory(radiance_assets::TEXTURE_MISSING_TEXTURE_FILE)
                .unwrap()
                .to_rgba8();
        let rgba_image = owned_image.as_ref().unwrap_or(&texture_missing);

        Self::from_buffer(
            rgba_image.as_raw(),
            0,
            rgba_image.width(),
            rgba_image.height(),
            device,
            allocator,
            command_runner,
        )
    }

    /// Upload `image_buffer` (`R8G8B8A8_UNORM`) into a Vulkan image
    /// with a full mipmap chain generated via a `vkCmdBlitImage` ladder
    /// inside a single one-shot command buffer (no extra submits).
    ///
    /// The bound sampler determines whether the chain is actually used:
    /// material samplers built via `VulkanSamplerCache::get_or_create_for`
    /// pass the texture's `mip_levels` so `max_lod` covers the whole
    /// chain; legacy paths that use `default_sampler()` (max_lod = 0,
    /// e.g. imgui font atlas) sample mip 0 only and are visually
    /// unchanged.
    ///
    /// Assumes `R8G8B8A8_UNORM` supports `BLIT_SRC | BLIT_DST |
    /// SAMPLED_IMAGE_FILTER_LINEAR` — guaranteed by the Vulkan
    /// "Required Format Support" table (§37.3) for color-renderable
    /// formats, so no runtime capability check is needed.
    pub fn from_buffer(
        image_buffer: &[u8],
        row_length: u32,
        width: u32,
        height: u32,
        device: &Rc<Device>,
        allocator: &Rc<vk_mem::Allocator>,
        command_runner: &Rc<AdhocCommandRunner>,
    ) -> Result<Self, Box<dyn Error>> {
        let buffer = Buffer::new_staging_buffer_with_data(allocator, &image_buffer)?;
        let format = vk::Format::R8G8B8A8_UNORM;
        let mip_levels = Image::full_mip_levels(width, height);
        let mut image = Image::new_color_image(allocator, width, height, mip_levels)?;

        // Batch the texture upload + mip generation into a single
        // one-shot command buffer. PAL3 scenes can reference hundreds
        // of textures at load time; previously every texture cost
        // three `vkQueueSubmit` + `vkQueueWaitIdle` round-trips, then
        // one after this change to Round 1; staying at one per
        // texture even with mip generation is the point.
        command_runner.run_commands_one_shot(|dev, cb| {
            // Step 1: bring mip 0 to TRANSFER_DST and copy the staging buffer.
            image.record_transit_layout_range(
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                0,
                1,
                dev,
                cb,
            );
            image.record_copy_from(&buffer, row_length, dev, cb);

            // Step 2: walk the chain. For each level i > 0: transition
            // level i-1 SRC, transition level i DST (UNDEFINED on
            // first use), blit i-1 → i, transition level i-1 to
            // SHADER_READ_ONLY.
            let mut src_w = width as i32;
            let mut src_h = height as i32;
            for level in 1..mip_levels {
                let dst_w = (src_w / 2).max(1);
                let dst_h = (src_h / 2).max(1);

                image.record_transit_layout_range(
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    level - 1,
                    1,
                    dev,
                    cb,
                );
                image.record_transit_layout_range(
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    level,
                    1,
                    dev,
                    cb,
                );
                image.record_blit_mip(
                    level - 1,
                    src_w,
                    src_h,
                    level,
                    dst_w,
                    dst_h,
                    dev,
                    cb,
                );
                image.record_transit_layout_range(
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    level - 1,
                    1,
                    dev,
                    cb,
                );

                src_w = dst_w;
                src_h = dst_h;
            }

            // Step 3: transition the last (or only) level. After the
            // loop the deepest level still sits in TRANSFER_DST_OPTIMAL.
            image.record_transit_layout_range(
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                mip_levels - 1,
                1,
                dev,
                cb,
            );
        })?;

        let image_view =
            ImageView::new_color_image_view(device.clone(), image.vk_image(), format, mip_levels)?;

        Ok(Self { image, image_view })
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn image_view(&self) -> &ImageView {
        &self.image_view
    }

    /// Mip-chain length of the underlying image. Material sampler
    /// creation uses this to pick a `max_lod` matching the chain.
    pub fn mip_levels(&self) -> u32 {
        self.image.mip_levels()
    }
}

pub struct VulkanTextureStore {
    store: LruCache<String, Rc<VulkanTexture>>,
}

impl VulkanTextureStore {
    pub fn new() -> Self {
        Self {
            store: LruCache::new(NonZero::new(10000).unwrap()),
        }
    }

    pub fn get_or_update(
        &mut self,
        name: &str,
        update: impl FnOnce() -> VulkanTexture,
    ) -> Rc<VulkanTexture> {
        if let Some(t) = self.store.get(name) {
            t.clone()
        } else {
            let t = Rc::new(update());
            self.store.put(name.to_string(), t.clone());
            t
        }
    }
}
