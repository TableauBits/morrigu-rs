use crate::{error::Error, utils::ThreadSafeRef};

use ash::{vk, Device};
use spirv_reflect::types::{ReflectBlockVariable, ReflectDescriptorBinding, ReflectDescriptorType};

use std::{fs, path::Path};

pub(crate) fn binding_type_cast(
    descriptor_type: ReflectDescriptorType,
) -> Result<vk::DescriptorType, &'static str> {
    match descriptor_type {
        ReflectDescriptorType::UniformBuffer => Ok(vk::DescriptorType::UNIFORM_BUFFER),
        ReflectDescriptorType::CombinedImageSampler => {
            Ok(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        }
        _ => Err("Unsupported binding type in shader"),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BindingData {
    pub set: u32,
    pub slot: u32,
    pub descriptor_type: ReflectDescriptorType,
    pub size: u32,
}

pub struct Shader {
    pub(crate) vertex_module: vk::ShaderModule,
    pub(crate) fragment_module: vk::ShaderModule,

    pub(crate) level_2_dsl: vk::DescriptorSetLayout,
    pub(crate) level_3_dsl: vk::DescriptorSetLayout,

    pub vertex_bindings: Vec<BindingData>,
    pub vertex_push_constants: Vec<ReflectBlockVariable>,
    pub fragment_bindings: Vec<BindingData>,
    pub fragment_push_constants: Vec<ReflectBlockVariable>,
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

pub(crate) fn create_shader_module(
    device: &Device,
    source: &[u32],
) -> Result<vk::ShaderModule, vk::Result> {
    let module_info = vk::ShaderModuleCreateInfo::builder().code(source);

    unsafe { device.create_shader_module(&module_info, None) }
}

impl Shader {
    /// This function expects a valid path for both **SPIR-V compiled** shader files.
    pub fn from_path(
        vertex_path: &Path,
        fragment_path: &Path,
        device: &Device,
    ) -> Result<ThreadSafeRef<Self>, Error> {
        let vertex_spirv = fs::read(vertex_path)?;
        let fragment_spirv = fs::read(fragment_path)?;

        Self::from_spirv_u8(&vertex_spirv, &fragment_spirv, device)
    }

    /// This function expects **COMPILED SPIR-V**, not higher level languages like GLSL or HSLS source code.
    pub fn from_spirv_u8(
        vertex_spirv: &[u8],
        fragment_spirv: &[u8],
        device: &Device,
    ) -> Result<ThreadSafeRef<Self>, Error> {
        let vertex_u32 = ash::util::read_spv(&mut std::io::Cursor::new(vertex_spirv))?;
        let fragment_u32 = ash::util::read_spv(&mut std::io::Cursor::new(fragment_spirv))?;

        Self::from_spirv_u32(device, &vertex_u32, &fragment_u32)
    }

    /// This function expects **COMPILED SPIR-V**, not higher level languages like GLSL or HSLS source code.
    pub fn from_spirv_u32(
        device: &Device,
        vertex_spirv: &[u32],
        fragment_spirv: &[u32],
    ) -> Result<ThreadSafeRef<Self>, Error> {
        let vertex_module = create_shader_module(device, vertex_spirv)?;
        let fragment_module = create_shader_module(device, fragment_spirv)?;

        let vertex_reflection_module = spirv_reflect::ShaderModule::load_u32_data(vertex_spirv)?;
        let vertex_entry_point = vertex_reflection_module.enumerate_entry_points()?[0].clone();
        let vertex_bindings_reflection = vertex_reflection_module
            .enumerate_descriptor_bindings(Some(vertex_entry_point.name.as_str()))?;
        let vertex_push_constants = vertex_reflection_module
            .enumerate_push_constant_blocks(Some(vertex_entry_point.name.as_str()))?;

        let fragment_reflection_module =
            spirv_reflect::ShaderModule::load_u32_data(fragment_spirv)?;
        let fragment_entry_point = fragment_reflection_module.enumerate_entry_points()?[0].clone();
        let fragment_bindings_reflection = fragment_reflection_module
            .enumerate_descriptor_bindings(Some(fragment_entry_point.name.as_str()))?;
        let fragment_push_constants = fragment_reflection_module
            .enumerate_push_constant_blocks(Some(fragment_entry_point.name.as_str()))?;

        let level_2_dsl = create_dsl(
            device,
            2,
            &[
                (vertex_bindings_reflection.clone(), vk::ShaderStageFlags::VERTEX),
                (fragment_bindings_reflection.clone(), vk::ShaderStageFlags::FRAGMENT),
            ],
        )?;
        let level_3_dsl = create_dsl(
            device,
            3,
            &[
                (vertex_bindings_reflection.clone(), vk::ShaderStageFlags::VERTEX),
                (fragment_bindings_reflection.clone(), vk::ShaderStageFlags::FRAGMENT),
            ],
        )?;

        let vertex_bindings = vertex_bindings_reflection
            .iter()
            .map(|binding| BindingData {
                set: binding.set,
                slot: binding.binding,
                descriptor_type: binding.descriptor_type,
                size: binding.block.size,
            })
            .collect::<Vec<_>>();
        let fragment_bindings = fragment_bindings_reflection
            .iter()
            .map(|binding| BindingData {
                set: binding.set,
                slot: binding.binding,
                descriptor_type: binding.descriptor_type,
                size: binding.block.size,
            })
            .collect::<Vec<_>>();

        Ok(ThreadSafeRef::new(Self {
            vertex_module,
            fragment_module,
            level_2_dsl,
            level_3_dsl,
            vertex_bindings,
            vertex_push_constants,
            fragment_bindings,
            fragment_push_constants,
        }))
    }

    pub fn destroy(&mut self, device: &Device) {
        unsafe {
            device.destroy_descriptor_set_layout(self.level_3_dsl, None);
            device.destroy_descriptor_set_layout(self.level_2_dsl, None);
            device.destroy_shader_module(self.fragment_module, None);
            device.destroy_shader_module(self.vertex_module, None);
        }
    }
}
