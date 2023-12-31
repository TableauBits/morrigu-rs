use ash::vk;
use thiserror::Error;

use crate::{
    allocated_types::{AllocatedBuffer, AllocatedImage, BufferBuildError},
    descriptor_resources::{
        DescriptorResources, DescriptorSetUpdateError, ResourceBindingError, UniformUpdateError,
    },
    material::{Material, Vertex},
    math_types::Mat4,
    mesh::Mesh,
    renderer::Renderer,
    texture::Texture,
    utils::ThreadSafeRef,
};

#[derive(Debug, bevy_ecs::prelude::Component)]
pub struct MeshRendering<VertexType>
where
    VertexType: Vertex,
{
    descriptor_pool: vk::DescriptorPool,
    pub descriptor_resources: DescriptorResources,

    pub mesh_ref: ThreadSafeRef<Mesh<VertexType>>,
    pub material_ref: ThreadSafeRef<Material<VertexType>>,

    pub(crate) descriptor_set: vk::DescriptorSet, // level 3
}

pub fn default_ubo_bindings(
    renderer: &mut Renderer,
) -> Result<(u32, ThreadSafeRef<AllocatedBuffer>), BufferBuildError> {
    let size: u64 = std::mem::size_of::<Mat4>().try_into().unwrap();
    Ok((
        0,
        ThreadSafeRef::new(AllocatedBuffer::builder(size).build(renderer)?),
    ))
}
pub fn default_descriptor_resources(
    renderer: &mut Renderer,
) -> Result<DescriptorResources, BufferBuildError> {
    Ok(DescriptorResources {
        uniform_buffers: [default_ubo_bindings(renderer)?].into(),
        ..Default::default()
    })
}

#[derive(Error, Debug)]
pub enum MeshRenderingBuildError {
    #[error("Material's vulkan descriptor pool creation failed with status: {0}.")]
    VulkanDescriptorPoolCreationFailed(vk::Result),

    #[error("Material's vulkan descriptor set allocation failed with status: {0}.")]
    VulkanDescriptorSetAllocationFailed(vk::Result),

    #[error("Material's descriptor set update failed with status: {0}.")]
    DescriptorSetUpdateFailed(#[from] DescriptorSetUpdateError),
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
    ) -> Result<ThreadSafeRef<Self>, MeshRenderingBuildError> {
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
        let descriptor_pool = unsafe { renderer.device.create_descriptor_pool(&pool_info, None) }
            .map_err(|result| {
            MeshRenderingBuildError::VulkanDescriptorPoolCreationFailed(result)
        })?;

        let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&material_shader.level_3_dsl));
        let descriptor_set = unsafe {
            renderer
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
        }
        .map_err(MeshRenderingBuildError::VulkanDescriptorSetAllocationFailed)?[0];

        let mut merged_bindings = material_shader.vertex_bindings.clone();
        merged_bindings.extend(&material_shader.fragment_bindings);
        descriptor_resources.update_descriptors_set_from_bindings(
            &merged_bindings,
            &descriptor_set,
            Some(&[3]),
            renderer,
        )?;

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

    pub fn bind_uniform(
        &mut self,
        binding_slot: u32,
        buffer_ref: ThreadSafeRef<AllocatedBuffer>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<AllocatedBuffer>, ResourceBindingError> {
        let Some(old_buffer) = self
            .descriptor_resources
            .uniform_buffers
            .insert(binding_slot, buffer_ref.clone())
        else {
            return Err(ResourceBindingError::InvalidBindingSlot {
                slot: binding_slot,
                set: 3,
            });
        };

        let buffer = buffer_ref.lock();

        let descriptor_buffer_info = vk::DescriptorBufferInfo::builder()
            .buffer(buffer.handle)
            .offset(0)
            .range(buffer.allocation.as_ref().unwrap().size());

        let set_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(binding_slot)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(std::slice::from_ref(&descriptor_buffer_info))
            .build();

        unsafe {
            renderer
                .device
                .update_descriptor_sets(std::slice::from_ref(&set_write), &[])
        };

        Ok(old_buffer)
    }

    pub fn update_uniform<T: bytemuck::Pod>(
        &mut self,
        binding_slot: u32,
        data: T,
    ) -> Result<(), UniformUpdateError> {
        self.descriptor_resources
            .uniform_buffers
            .get(&binding_slot)
            .ok_or(UniformUpdateError::InvalidBindingSlot {
                slot: binding_slot,
                set: 3,
            })?
            .lock()
            .upload_data(data)
            .map_err(|err| err.into())
    }

    pub fn bind_storage_image<T: bytemuck::Pod>(
        &mut self,
        binding_slot: u32,
        image_ref: ThreadSafeRef<AllocatedImage>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<AllocatedImage>, ResourceBindingError> {
        let Some(old_image) = self
            .descriptor_resources
            .storage_images
            .insert(binding_slot, image_ref.clone())
        else {
            return Err(ResourceBindingError::InvalidBindingSlot {
                slot: binding_slot,
                set: 3,
            });
        };

        let image = image_ref.lock();

        let descriptor_image_info = vk::DescriptorImageInfo::builder()
            .image_view(image.view)
            .image_layout(vk::ImageLayout::GENERAL);

        let set_write = vk::WriteDescriptorSet::builder()
            .dst_set(self.descriptor_set)
            .dst_binding(binding_slot)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(std::slice::from_ref(&descriptor_image_info))
            .build();

        unsafe {
            renderer
                .device
                .update_descriptor_sets(std::slice::from_ref(&set_write), &[])
        };

        Ok(old_image)
    }

    pub fn bind_texture(
        &mut self,
        binding_slot: u32,
        texture_ref: ThreadSafeRef<Texture>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Texture>, ResourceBindingError> {
        let Some(old_texture) = self
            .descriptor_resources
            .sampled_images
            .insert(binding_slot, texture_ref.clone())
        else {
            return Err(ResourceBindingError::InvalidBindingSlot {
                slot: binding_slot,
                set: 3,
            });
        };

        let texture = texture_ref.lock();

        let descriptor_image_info = vk::DescriptorImageInfo::builder()
            .sampler(texture.sampler)
            .image_view(texture.image_ref.lock().view)
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

        Ok(old_texture)
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        unsafe {
            renderer
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);
        }
    }
}
