use ash::prelude::VkResult;
use ash::vk;
use std::rc::Rc;

use super::device::Device;

pub struct ImageView {
    device: Rc<Device>,
    image_view: vk::ImageView,
}

impl ImageView {
    pub fn new_color_image_view(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
        mip_levels: u32,
    ) -> VkResult<Self> {
        Self::new(
            device,
            image,
            format,
            vk::ImageAspectFlags::COLOR,
            mip_levels,
        )
    }

    pub fn new_depth_image_view(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
    ) -> VkResult<Self> {
        Self::new(device, image, format, vk::ImageAspectFlags::DEPTH, 1)
    }

    /// Single-layer depth view into one layer of a depth array image. Used as a
    /// per-cascade framebuffer attachment for the CSM depth passes.
    pub fn new_depth_layer_view(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
        layer: u32,
    ) -> VkResult<Self> {
        Self::new_layered(
            device,
            image,
            format,
            vk::ImageAspectFlags::DEPTH,
            layer,
            1,
            vk::ImageViewType::TYPE_2D,
        )
    }

    /// `2DArray` depth view spanning `layer_count` layers. Used as the sampled
    /// CSM texture (the lit shaders index it by cascade).
    pub fn new_depth_array_view(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
        layer_count: u32,
    ) -> VkResult<Self> {
        Self::new_layered(
            device,
            image,
            format,
            vk::ImageAspectFlags::DEPTH,
            0,
            layer_count,
            vk::ImageViewType::TYPE_2D_ARRAY,
        )
    }

    pub fn vk_image_view(&self) -> vk::ImageView {
        self.image_view
    }

    fn new(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
        aspect_mask: vk::ImageAspectFlags,
        mip_levels: u32,
    ) -> VkResult<Self> {
        let component_mapping = vk::ComponentMapping::default()
            .a(vk::ComponentSwizzle::IDENTITY)
            .r(vk::ComponentSwizzle::IDENTITY)
            .g(vk::ComponentSwizzle::IDENTITY)
            .b(vk::ComponentSwizzle::IDENTITY);
        let subres_range = vk::ImageSubresourceRange::default()
            .aspect_mask(aspect_mask)
            .base_array_layer(0)
            .layer_count(1)
            .base_mip_level(0)
            .level_count(mip_levels.max(1));
        let create_info = vk::ImageViewCreateInfo::default()
            .format(format)
            .image(image)
            .components(component_mapping)
            .subresource_range(subres_range)
            .view_type(vk::ImageViewType::TYPE_2D);
        let view = device.create_image_view(&create_info)?;
        Ok(Self {
            device,
            image_view: view,
        })
    }

    fn new_layered(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
        aspect_mask: vk::ImageAspectFlags,
        base_array_layer: u32,
        layer_count: u32,
        view_type: vk::ImageViewType,
    ) -> VkResult<Self> {
        let component_mapping = vk::ComponentMapping::default()
            .a(vk::ComponentSwizzle::IDENTITY)
            .r(vk::ComponentSwizzle::IDENTITY)
            .g(vk::ComponentSwizzle::IDENTITY)
            .b(vk::ComponentSwizzle::IDENTITY);
        let subres_range = vk::ImageSubresourceRange::default()
            .aspect_mask(aspect_mask)
            .base_array_layer(base_array_layer)
            .layer_count(layer_count.max(1))
            .base_mip_level(0)
            .level_count(1);
        let create_info = vk::ImageViewCreateInfo::default()
            .format(format)
            .image(image)
            .components(component_mapping)
            .subresource_range(subres_range)
            .view_type(view_type);
        let view = device.create_image_view(&create_info)?;
        Ok(Self {
            device,
            image_view: view,
        })
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        self.device.destroy_image_view(self.image_view);
    }
}
