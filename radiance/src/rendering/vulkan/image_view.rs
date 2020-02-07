use ash::prelude::VkResult;
use ash::version::DeviceV1_0;
use ash::{vk, Device};
use std::rc::{Rc, Weak};

pub struct ImageView {
    device: Weak<Device>,
    image_view: vk::ImageView,
}

impl ImageView {
    pub fn new(device: &Rc<Device>, image: vk::Image, format: vk::Format) -> VkResult<Self> {
        let component_mapping = vk::ComponentMapping::builder()
            .a(vk::ComponentSwizzle::IDENTITY)
            .r(vk::ComponentSwizzle::IDENTITY)
            .g(vk::ComponentSwizzle::IDENTITY)
            .b(vk::ComponentSwizzle::IDENTITY)
            .build();
        let subres_range = vk::ImageSubresourceRange::builder()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
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
        let view = unsafe { device.create_image_view(&create_info, None)? };
        Ok(Self {
            device: Rc::downgrade(device),
            image_view: view,
        })
    }

    pub fn vk_image_view(&self) -> vk::ImageView {
        self.image_view
    }
}

impl Drop for ImageView {
    fn drop(&mut self) {
        let device = self.device.upgrade().unwrap();
        unsafe {
            device.destroy_image_view(self.image_view, None);
        }
    }
}
