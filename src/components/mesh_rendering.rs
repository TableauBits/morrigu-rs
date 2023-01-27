use ash::vk;
use bytemuck::bytes_of;

use crate::{
    descriptor_resources::{generate_descriptors_write_from_bindings, DescriptorResources},
    error::Error,
    material::{Material, Vertex},
    mesh::Mesh,
    renderer::Renderer,
    texture::Texture,
    utils::ThreadSafeRef,
};

#[derive(bevy_ecs::prelude::Component)]
pub struct MeshRendering<VertexType>
where
    VertexType: Vertex,
{
    descriptor_pool: vk::DescriptorPool,
    descriptor_resources: DescriptorResources,

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
        descriptor_resources: DescriptorResources,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Self>, Error> {
        let mesh_ref = ThreadSafeRef::clone(mesh_ref);
        let mesh = mesh_ref.lock();

        let material_ref = ThreadSafeRef::clone(material_ref);
        let material = material_ref.lock();

        let material_shader = material.shader_ref.lock();
        let ubo_count: u32 = descriptor_resources
            .uniform_buffers
            .len()
            .try_into()
            .unwrap();
        let storage_image_count: u32 = descriptor_resources
            .storage_images
            .len()
            .try_into()
            .unwrap();
        let sampled_image_count: u32 = descriptor_resources
            .sampled_images
            .len()
            .try_into()
            .unwrap();

        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: std::cmp::max(ubo_count, 1),
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: std::cmp::max(storage_image_count, 1),
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

        let mut merged_bindings = material_shader.vertex_bindings.clone();
        merged_bindings.extend(&material_shader.fragment_bindings);
        let descriptor_writes = generate_descriptors_write_from_bindings(
            &merged_bindings,
            &descriptor_set,
            Some(&[2]),
            &descriptor_resources,
        )?;

        unsafe {
            renderer
                .device
                .update_descriptor_sets(&descriptor_writes, &[])
        };

        drop(material_shader);
        drop(material);
        drop(mesh);

        Ok(ThreadSafeRef::new(Self {
            descriptor_pool,
            descriptor_resources,
            mesh_ref,
            material_ref,
            descriptor_set,
        }))
    }

    pub fn upload_uniform<T: bytemuck::Pod>(
        &mut self,
        binding_slot: u32,
        data: T,
    ) -> Result<(), Error> {
        let binding_data = self
            .descriptor_resources
            .uniform_buffers
            .get_mut(&binding_slot)
            .ok_or_else(|| format!("no slot {} to bind to", binding_slot))?;

        let allocation = binding_data.allocation.as_mut().ok_or("use after free")?;

        if allocation.size() < std::mem::size_of::<T>().try_into()? {
            return Err(format!(
                "invalid size {} (expected {}) (make sure T is #[repr(C)]",
                std::mem::size_of::<T>(),
                allocation.size(),
            )
            .into());
        }

        let raw_data = bytes_of(&data);
        allocation
            .mapped_slice_mut()
            .ok_or("failed to map memory")?[..raw_data.len()]
            .copy_from_slice(raw_data);

        Ok(())
    }

    pub fn bind_texture(
        &mut self,
        binding_slot: u32,
        texture_ref: &ThreadSafeRef<Texture>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Texture>, Error> {
        if !self
            .descriptor_resources
            .sampled_images
            .contains_key(&binding_slot)
        {
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
            .descriptor_resources
            .sampled_images
            .insert(binding_slot, texture_ref)
            .unwrap())
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        unsafe {
            for uniform in self.descriptor_resources.uniform_buffers.values_mut() {
                uniform.destroy(&renderer.device, &mut renderer.allocator());
            }
            for storage_image in self.descriptor_resources.storage_images.values_mut() {
                storage_image.destroy(renderer);
            }

            renderer
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}
