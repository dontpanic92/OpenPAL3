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
        let mut image = Image::new_color_image(allocator, width, height)?;
        // Batch the three texture-upload steps (UNDEFINED -> TRANSFER_DST
        // barrier, buffer-to-image copy, TRANSFER_DST -> SHADER_READ_ONLY
        // barrier) into a single one-shot command buffer instead of one
        // submit per step. PAL3 scenes can reference hundreds of
        // textures at load time; previously every texture cost three
        // `vkQueueSubmit` + `vkQueueWaitIdle` round-trips.
        command_runner.run_commands_one_shot(|dev, cb| {
            image.record_transit_layout(
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                dev,
                cb,
            );
            image.record_copy_from(&buffer, row_length, dev, cb);
            image.record_transit_layout(
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                dev,
                cb,
            );
        })?;

        let image_view = ImageView::new_color_image_view(device.clone(), image.vk_image(), format)?;

        Ok(Self { image, image_view })
    }

    pub fn image(&self) -> &Image {
        &self.image
    }

    pub fn image_view(&self) -> &ImageView {
        &self.image_view
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
