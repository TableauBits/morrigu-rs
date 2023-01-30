use crate::{
    allocated_types::{AllocatedBuffer, AllocatedImage},
    renderer::Renderer,
    shader::BindingData,
    texture::Texture,
    utils::ThreadSafeRef,
};

use std::collections::HashMap;

use ash::{vk, Device};
use spirv_reflect::types::{ReflectDescriptorBinding, ReflectDescriptorType};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("Unsupported descriptor type detected in shader: {0:?}.")]
pub struct UnsupportedDescriptorTypeError(ReflectDescriptorType);

pub(crate) fn binding_type_cast(
    descriptor_type: ReflectDescriptorType,
) -> Result<vk::DescriptorType, UnsupportedDescriptorTypeError> {
    match descriptor_type {
        ReflectDescriptorType::UniformBuffer => Ok(vk::DescriptorType::UNIFORM_BUFFER),
        ReflectDescriptorType::StorageImage => Ok(vk::DescriptorType::STORAGE_IMAGE),
        ReflectDescriptorType::CombinedImageSampler => {
            Ok(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        }
        _ => Err(UnsupportedDescriptorTypeError(descriptor_type)),
    }
}

#[derive(Error, Debug)]
pub enum DSLCreationError {
    #[error("Unsupported binding type detected in shader: {0:?}.")]
    UnsupportedDescriptorType(#[from] UnsupportedDescriptorTypeError),

    #[error("Vulkan creating of descriptor set layout failed with VkResult: {0}.")]
    VulkanError(#[from] vk::Result),
}

pub(crate) fn create_dsl(
    device: &Device,
    set_level: u32,
    stage_bindings: &[(Vec<ReflectDescriptorBinding>, vk::ShaderStageFlags)],
) -> Result<vk::DescriptorSetLayout, DSLCreationError> {
    let mut bindings_infos = vec![];

    let mut ubo_map = HashMap::new();
    let mut images_map = HashMap::new();
    let mut sampler_map = HashMap::new();

    for (bindings, stage) in stage_bindings {
        for binding_reflection in bindings {
            if binding_reflection.set != set_level {
                continue;
            }

            let binding_type = binding_type_cast(binding_reflection.descriptor_type)?;
            let map = match binding_type {
                vk::DescriptorType::UNIFORM_BUFFER => Ok(&mut ubo_map),
                vk::DescriptorType::STORAGE_IMAGE => Ok(&mut images_map),
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER => Ok(&mut sampler_map),
                _ => Err(UnsupportedDescriptorTypeError(
                    binding_reflection.descriptor_type,
                )),
            }?;

            match map.get(&binding_reflection.binding) {
                None => {
                    let set_binding = vk::DescriptorSetLayoutBinding {
                        binding: binding_reflection.binding,
                        descriptor_type: binding_type,
                        descriptor_count: 1,
                        stage_flags: *stage,
                        ..Default::default()
                    };

                    map.insert(binding_reflection.binding, set_binding);
                }
                Some(&old_binding) => {
                    let mut new_binding = old_binding;
                    new_binding.stage_flags |= *stage;
                    map.insert(binding_reflection.binding, new_binding);
                }
            }
        }
    }

    for (_, binding_info) in ubo_map {
        bindings_infos.push(binding_info);
    }
    for (_, binding_info) in sampler_map {
        bindings_infos.push(binding_info);
    }

    let dsl_create_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&bindings_infos);

    Ok(unsafe { device.create_descriptor_set_layout(&dsl_create_info, None)? })
}

#[derive(Error, Debug)]
pub enum DescriptorSetUpdateError {
    #[error("Unsupported binding type detected in shader: {0:?}.")]
    UnsupportedDescriptorType(#[from] UnsupportedDescriptorTypeError),

    #[error("Required shader resource at binding {set} and location {slot} was not provided.")]
    ResourceNotProvided { set: u32, slot: u32 },
}

pub(crate) fn update_descriptors_set_from_bindings(
    bindings: &[BindingData],
    descriptor_set: &vk::DescriptorSet,
    set_constraints: Option<&[u32]>,
    resources: &DescriptorResources,
    renderer: &mut Renderer,
) -> Result<(), DescriptorSetUpdateError> {
    for binding in bindings {
        if let Some(set_constraints) = set_constraints {
            if !set_constraints.contains(&binding.set) {
                continue;
            }
        }

        match binding_type_cast(binding.descriptor_type)? {
            vk::DescriptorType::UNIFORM_BUFFER => {
                let buffer_ref = resources.uniform_buffers.get(&binding.slot).ok_or(
                    DescriptorSetUpdateError::ResourceNotProvided {
                        set: binding.set,
                        slot: binding.slot,
                    },
                )?;
                let buffer = buffer_ref.lock();

                let descriptor_buffer_info = vk::DescriptorBufferInfo::builder()
                    .buffer(buffer.handle)
                    .offset(0)
                    .range(buffer.size());

                let set_write = vk::WriteDescriptorSet::builder()
                    .dst_set(*descriptor_set)
                    .dst_binding(binding.slot)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(std::slice::from_ref(&descriptor_buffer_info));

                unsafe { renderer.device.update_descriptor_sets(&[*set_write], &[]) };
            }
            vk::DescriptorType::STORAGE_IMAGE => {
                let image_ref = resources.storage_images.get(&binding.slot).ok_or(
                    DescriptorSetUpdateError::ResourceNotProvided {
                        set: binding.set,
                        slot: binding.slot,
                    },
                )?;
                let image = image_ref.lock();

                let descriptor_image_info = vk::DescriptorImageInfo::builder()
                    .image_view(image.view)
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

                let set_write = vk::WriteDescriptorSet::builder()
                    .dst_set(*descriptor_set)
                    .dst_binding(binding.slot)
                    .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                    .image_info(std::slice::from_ref(&descriptor_image_info));

                unsafe { renderer.device.update_descriptor_sets(&[*set_write], &[]) };
            }
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER => {
                let texture_ref = resources.sampled_images.get(&binding.slot).ok_or(
                    DescriptorSetUpdateError::ResourceNotProvided {
                        set: binding.set,
                        slot: binding.slot,
                    },
                )?;
                let texture = texture_ref.lock();

                let descriptor_image_info = vk::DescriptorImageInfo::builder()
                    .sampler(texture.sampler)
                    .image_view(texture.image_ref.lock().view)
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

                let set_write = vk::WriteDescriptorSet::builder()
                    .dst_set(*descriptor_set)
                    .dst_binding(binding.slot)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&descriptor_image_info));

                unsafe { renderer.device.update_descriptor_sets(&[*set_write], &[]) };
            }
            _ => Err(UnsupportedDescriptorTypeError(binding.descriptor_type))?,
        };
    }

    Ok(())
}

#[derive(Debug, Default)]
pub struct DescriptorResources {
    pub uniform_buffers: HashMap<u32, ThreadSafeRef<AllocatedBuffer>>,
    pub storage_images: HashMap<u32, ThreadSafeRef<AllocatedImage>>,
    pub sampled_images: HashMap<u32, ThreadSafeRef<Texture>>,
}

impl DescriptorResources {
    /// Returns a completely empty descriptor set resource structure. This cannot be used with
    /// graphics mesh rendering component, as it requires at least a uniform at `location = 0` for
    /// the model matrix.
    pub fn empty() -> Self {
        Self::default()
    }
}
