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
    ) -> VkResult<Self> {
        Self::new(device, image, format, vk::ImageAspectFlags::COLOR)
    }

    pub fn new_depth_image_view(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
    ) -> VkResult<Self> {
        Self::new(device, image, format, vk::ImageAspectFlags::DEPTH)
    }

    pub fn vk_image_view(&self) -> vk::ImageView {
        self.image_view
    }

    fn new(
        device: Rc<Device>,
        image: vk::Image,
        format: vk::Format,
        aspect_mask: vk::ImageAspectFlags,
    ) -> VkResult<Self> {
        let component_mapping = vk::ComponentMapping::builder()
            .a(vk::ComponentSwizzle::IDENTITY)
            .r(vk::ComponentSwizzle::IDENTITY)
            .g(vk::ComponentSwizzle::IDENTITY)
            .b(vk::ComponentSwizzle::IDENTITY)
            .build();
        let subres_range = vk::ImageSubresourceRange::builder()
            .aspect_mask(aspect_mask)
            .base_array_layer(0)
            .layer_count(1)
            .base_mip_level(0)
            .level_count(1)
            .build();
        let create_info = vk::ImageViewCreateInfo::builder()
            .format(format)
            .image(image)
            .components(component_mapping)
            .subresource_range(subres_range)
            .view_type(vk::ImageViewType::TYPE_2D)
            .build();
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
