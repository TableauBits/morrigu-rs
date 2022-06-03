use ash::vk;

use crate::{
    allocated_types::AllocatedBuffer,
    error::Error,
    material::{Material, Vertex},
    mesh::Mesh,
    renderer::Renderer,
    shader::binding_type_cast,
    texture::Texture,
    utils::ThreadSafeRef,
};

#[derive(bevy_ecs::prelude::Component)]
pub struct MeshRendering<VertexType>
where
    VertexType: Vertex,
{
    descriptor_pool: vk::DescriptorPool,
    uniform_buffers: std::collections::HashMap<u32, AllocatedBuffer>,
    sampled_images: std::collections::HashMap<u32, ThreadSafeRef<Texture>>,

    pub mesh_ref: ThreadSafeRef<Mesh<VertexType>>,
    pub material_ref: ThreadSafeRef<Material<VertexType>>,

    pub(crate) descriptor_set: vk::DescriptorSet, // level 3
}

impl<VertexType> MeshRendering<VertexType>
where
    VertexType: Vertex,
{
    pub fn new(
        mesh_ref: &ThreadSafeRef<Mesh<VertexType>>,
        material_ref: &ThreadSafeRef<Material<VertexType>>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Self>, Error> {
        let mesh_ref = ThreadSafeRef::clone(mesh_ref);
        let mesh = mesh_ref.lock();

        let material_ref = ThreadSafeRef::clone(material_ref);
        let material = material_ref.lock();

        let mut ubo_count = 0;
        let mut sampled_image_count = 0;

        let material_shader = material.shader_ref.lock();

        for binding in material_shader
            .vertex_bindings
            .iter()
            .chain(material_shader.fragment_bindings.iter())
        {
            if binding.set != 3 {
                continue;
            }

            match binding_type_cast(binding.descriptor_type)? {
                vk::DescriptorType::UNIFORM_BUFFER => ubo_count += 1,
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER => sampled_image_count += 1,
                _ => (),
            }
        }

        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: std::cmp::max(ubo_count, 1),
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: std::cmp::max(sampled_image_count, 1),
            },
        ];
        let pool_info = vk::DescriptorPoolCreateInfo::builder()
            .max_sets(1)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = unsafe { renderer.device.create_descriptor_pool(&pool_info, None) }?;

        let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&material_shader.level_3_dsl));
        let descriptor_set = unsafe {
            renderer
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
        }?[0];

        let mut uniform_buffers = std::collections::HashMap::new();
        let mut sampled_images = std::collections::HashMap::new();

        for binding in material_shader
            .vertex_bindings
            .iter()
            .chain(material_shader.fragment_bindings.iter())
        {
            if binding.set != 3 {
                continue;
            }

            match binding_type_cast(binding.descriptor_type)? {
                vk::DescriptorType::UNIFORM_BUFFER => {
                    let buffer = AllocatedBuffer::builder(binding.size.into())
                        .with_usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                        .with_memory_location(gpu_allocator::MemoryLocation::CpuToGpu)
                        .build(&renderer.device, &mut renderer.allocator())?;
                    let descriptor_buffer_info = vk::DescriptorBufferInfo::builder()
                        .buffer(buffer.handle)
                        .offset(0)
                        .range(binding.size.into());
                    let set_write = vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(binding.slot)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .buffer_info(std::slice::from_ref(&descriptor_buffer_info));

                    unsafe {
                        renderer
                            .device
                            .update_descriptor_sets(std::slice::from_ref(&set_write), &[])
                    };
                    uniform_buffers.insert(binding.slot, buffer);
                }
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER => {
                    let texture_ref = Texture::builder().build_default(renderer)?;
                    let texture = texture_ref.lock();

                    let descriptor_image_info = vk::DescriptorImageInfo::builder()
                        .sampler(texture.sampler)
                        .image_view(texture.image.view)
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                    let set_write = vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(binding.slot)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .image_info(std::slice::from_ref(&descriptor_image_info));

                    unsafe {
                        renderer
                            .device
                            .update_descriptor_sets(std::slice::from_ref(&set_write), &[])
                    };

                    drop(texture);

                    sampled_images.insert(binding.slot, texture_ref);
                }
                _ => (),
            }
        }

        drop(material_shader);
        drop(material);
        drop(mesh);

        Ok(ThreadSafeRef::new(Self {
            descriptor_pool,
            uniform_buffers,
            sampled_images,
            mesh_ref,
            material_ref,
            descriptor_set,
        }))
    }

    pub fn upload_uniform<T>(&self, binding_slot: u32, data: T) -> Result<(), Error> {
        let binding_data = self
            .uniform_buffers
            .get(&binding_slot)
            .ok_or_else(|| format!("no slot {} to bind to", binding_slot))?;
        let allocation = binding_data.allocation.as_ref().ok_or("use after free")?;

        if allocation.size() < std::mem::size_of::<T>().try_into()? {
            return Err(format!(
                "invalid size {} (expected {}) (make sure T is #[repr(C)]",
                std::mem::size_of::<T>(),
                allocation.size(),
            )
            .into());
        }

        let dst = allocation
            .mapped_ptr()
            .ok_or("failed to map memory")?
            .cast::<T>()
            .as_ptr();
        unsafe { std::ptr::copy_nonoverlapping(&data, dst, 1) };

        Ok(())
    }

    pub fn bind_texture(
        &mut self,
        binding_slot: u32,
        texture_ref: &ThreadSafeRef<Texture>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Texture>, Error> {
        if !self.sampled_images.contains_key(&binding_slot) {
            return Err("Invalid binding slot".into());
        };

        let texture_ref = ThreadSafeRef::clone(texture_ref);
        let texture = texture_ref.lock();

        let descriptor_image_info = vk::DescriptorImageInfo::builder()
            .sampler(texture.sampler)
            .image_view(texture.image.view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

        let set_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(binding_slot)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .image_info(std::slice::from_ref(&descriptor_image_info));

        unsafe {
            renderer
                .device
                .update_descriptor_sets(std::slice::from_ref(&set_write), &[])
        };

        drop(texture);

        Ok(self
            .sampled_images
            .insert(binding_slot, texture_ref)
            .unwrap())
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        unsafe {
            for uniform in self.uniform_buffers.values_mut() {
                uniform.destroy(&renderer.device, &mut renderer.allocator());
            }

            // Not sure if we should destroy those
            // for image in self.sampled_images.values_mut() {
            // let mut image = image.lock();
            // image.destroy(renderer);
            // }

            renderer
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}
