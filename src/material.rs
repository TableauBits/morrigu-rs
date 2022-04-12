use ash::vk;
use gpu_allocator::vulkan::Allocator;

use crate::{allocated_types::AllocatedBuffer, shader::Shader};

pub struct Material<'a> {
    pub pipeline: vk::Pipeline,
    pub layout: vk::PipelineLayout,
    pub level_2_descriptor: vk::DescriptorSet,

    pub shader: &'a Shader,

    uniform_buffers: std::collections::HashMap<u32, AllocatedBuffer>,
    // uniform_buffers: std::collections::HashMap<u32, Texture>,
    descriptor_pool: vk::DescriptorPool,
}

impl<'a> Material<'a> {
    pub fn destroy(self, device: &ash::Device, allocator: &mut Allocator) {
        unsafe {
            for (_, uniform) in self.uniform_buffers {
                uniform.destroy(device, allocator);
            }

            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}
