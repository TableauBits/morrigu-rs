use ash::vk;
use nalgebra_glm as glm;

use crate::material::{Vertex, VertexInputDescription};

pub struct TexturedVertex {
    position: glm::Vec3,
    normal: glm::Vec3,
    texture_coords: glm::Vec3,
}

impl Vertex for TexturedVertex {
    fn vertex_input_description() -> VertexInputDescription {
        let main_binding = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(
                std::mem::size_of::<TexturedVertex>()
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .input_rate(vk::VertexInputRate::VERTEX)
            .build();

        let position = vk::VertexInputAttributeDescription::builder()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(
                memoffset::offset_of!(TexturedVertex, position)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        let normal = vk::VertexInputAttributeDescription::builder()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(
                memoffset::offset_of!(TexturedVertex, normal)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        let texture_coords = vk::VertexInputAttributeDescription::builder()
            .location(2)
            .binding(0)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(
                memoffset::offset_of!(TexturedVertex, texture_coords)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        VertexInputDescription {
            bindings: vec![main_binding],
            attributes: vec![position, normal, texture_coords],
        }
    }
}
