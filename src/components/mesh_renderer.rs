use ash::vk;

use crate::{
    allocated_types::AllocatedBuffer,
    error::Error,
    material::{Material, Vertex},
    mesh::Mesh,
    renderer::Renderer,
    shader::binding_type_cast,
    texture::Texture,
};

#[derive(bevy_ecs::prelude::Component)]
pub struct MeshRenderer<'a, VertexType>
where
    VertexType: Vertex,
{
    descriptor_pool: vk::DescriptorPool,
    uniform_buffers: std::collections::HashMap<u32, AllocatedBuffer>,
    sampled_images: std::collections::HashMap<u32, Texture>,

    pub mesh: &'a Mesh<VertexType>,
    pub material: &'a Material<'a, VertexType>,

    pub(crate) descriptor_set: vk::DescriptorSet, // level 3
}

impl<'a, VertexType> MeshRenderer<'a, VertexType>
where
    VertexType: Vertex,
{
    pub fn new(
        mesh: &'a Mesh<VertexType>,
        material: &'a Material<VertexType>,
        renderer: &mut Renderer,
    ) -> Result<Self, Error> {
        let mut ubo_count = 0;
        let mut sampled_image_count = 0;

        for binding in material
            .shader
            .vertex_bindings
            .iter()
            .chain(material.shader.fragment_bindings.iter())
        {
            if binding.set != 3 {
                continue;
            }

            match binding_type_cast(binding.descriptor_type)? {
                vk::DescriptorType::UNIFORM_BUFFER => ubo_count += 1,
                vk::DescriptorType::SAMPLED_IMAGE => sampled_image_count += 1,
                _ => (),
            }
        }

        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: std::cmp::max(ubo_count, 1),
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: std::cmp::max(sampled_image_count, 1),
            },
        ];
        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .max_sets(1)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = unsafe { renderer.device.create_descriptor_pool(&pool_info, None) }?;

        let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&material.shader.level_3_dsl));
        let descriptor_set = unsafe {
            renderer
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
        }?[0];

        let mut uniform_buffers = std::collections::HashMap::new();
        let mut sampled_images = std::collections::HashMap::new();

        for binding in material
            .shader
            .vertex_bindings
            .iter()
            .chain(material.shader.fragment_bindings.iter())
        {
            if binding.set != 3 {
                continue;
            }

            match binding_type_cast(binding.descriptor_type)? {
                vk::DescriptorType::UNIFORM_BUFFER => {
                    let buffer = AllocatedBuffer::builder(binding.block.size.into())
                        .with_usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                        .with_memory_location(gpu_allocator::MemoryLocation::CpuToGpu)
                        .build(
                            &renderer.device,
                            renderer
                                .allocator
                                .as_mut()
                                .ok_or("Unintialized allocator")?,
                        )?;
                    let descriptor_buffer_info = vk::DescriptorBufferInfo::builder()
                        .buffer(buffer.handle)
                        .offset(0)
                        .range(binding.block.size.into());
                    let set_write = vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(binding.binding)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(std::slice::from_ref(&descriptor_buffer_info));

                    unsafe {
                        renderer
                            .device
                            .update_descriptor_sets(std::slice::from_ref(&set_write), &[])
                    };
                    uniform_buffers.insert(binding.binding, buffer);
                }
                vk::DescriptorType::SAMPLED_IMAGE => {
                    let texture = Texture::default(renderer)?;
                    let descriptor_image_info = vk::DescriptorImageInfo::builder()
                        .sampler(texture.sampler)
                        .image_view(texture.image.view)
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    let set_write = vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(binding.binding)
                        .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                        .image_info(std::slice::from_ref(&descriptor_image_info));

                    unsafe {
                        renderer
                            .device
                            .update_descriptor_sets(std::slice::from_ref(&set_write), &[])
                    };
                    sampled_images.insert(binding.binding, texture);
                }
                _ => (),
            }
        }

        Ok(Self {
            descriptor_pool,
            uniform_buffers,
            sampled_images,
            mesh,
            material,
            descriptor_set,
        })
    }

    pub fn destroy(self, renderer: &mut Renderer) {
        unsafe {
            for (_, uniform) in self.uniform_buffers {
                uniform.destroy(&renderer.device, renderer.allocator.as_mut().unwrap());
            }

            for (_, image) in self.sampled_images {
                image.destroy(renderer);
            }

            renderer
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}
