use crate::{
    allocated_types::{AllocatedBuffer, AllocatedImage},
    shader::BindingData,
    texture::Texture,
    utils::ThreadSafeRef,
};

use std::collections::HashMap;

use ash::vk;
use spirv_reflect::types::ReflectDescriptorType;

pub(crate) fn binding_type_cast(
    descriptor_type: ReflectDescriptorType,
) -> Result<vk::DescriptorType, &'static str> {
    match descriptor_type {
        ReflectDescriptorType::UniformBuffer => Ok(vk::DescriptorType::UNIFORM_BUFFER),
        ReflectDescriptorType::CombinedImageSampler => {
            Ok(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        }
        ReflectDescriptorType::StorageImage => Ok(vk::DescriptorType::STORAGE_IMAGE),
        _ => Err("Unsupported binding type in shader"),
    }
}

pub(crate) fn create_dsl(
    device: &Device,
    set_level: u32,
    stage_bindings: &[(Vec<ReflectDescriptorBinding>, vk::ShaderStageFlags)],
) -> Result<vk::DescriptorSetLayout, Error> {
    let mut bindings_infos = vec![];

    let mut ubo_map = std::collections::HashMap::new();
    let mut sampler_map = std::collections::HashMap::new();

    for (bindings, stage) in stage_bindings {
        for binding_reflection in bindings {
            if binding_reflection.set != set_level {
                continue;
            }

            let binding_type = binding_type_cast(binding_reflection.descriptor_type)?;
            let map = match binding_type {
                vk::DescriptorType::UNIFORM_BUFFER => Ok(&mut ubo_map),
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER => Ok(&mut sampler_map),
                _ => Err("Unsupported binding type in shader"),
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

pub(crate) fn generate_descriptors_write_form_binding(
    binding: &BindingData,
    resources: &DescriptorResources,
) {
}

pub struct DescriptorResources {
    pub uniform_buffers: HashMap<u32, AllocatedBuffer>,
    pub storage_images: HashMap<u32, AllocatedImage>,
    pub sampled_images: HashMap<u32, ThreadSafeRef<Texture>>,
}
