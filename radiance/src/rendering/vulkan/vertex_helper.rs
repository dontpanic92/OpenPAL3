use crate::rendering::Vertex;
use ash::vk;
use std::mem::size_of;

pub fn get_binding_description() -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription::builder()
        .binding(0)
        .stride(size_of::<Vertex>() as u32)
        .input_rate(vk::VertexInputRate::VERTEX)
        .build()
}

pub fn get_attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
    let pos_attr = vk::VertexInputAttributeDescription::builder()
        .offset(Vertex::position_offset() as u32)
        .binding(0)
        .location(0)
        .format(vk::Format::R32G32B32_SFLOAT)
        .build();

    let color_attr = vk::VertexInputAttributeDescription::builder()
        .offset(Vertex::color_offset() as u32)
        .binding(0)
        .location(1)
        .format(vk::Format::R32G32B32_SFLOAT)
        .build();

    [pos_attr, color_attr]
}
