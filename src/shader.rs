use crate::{
    descriptor_resources::{create_dsl, DSLCreationError},
    utils::ThreadSafeRef,
};

use ash::{vk, Device};
use spirv_reflect::types::{ReflectBlockVariable, ReflectDescriptorType, ReflectDimension};
use thiserror::Error;

use std::{fs, path::Path};

#[derive(Debug, Clone, Copy)]
pub struct BindingData {
    pub set: u32,
    pub slot: u32,
    pub descriptor_type: ReflectDescriptorType,
    pub size: u32,
    pub dim: ReflectDimension,
}

#[derive(Debug)]
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

pub(crate) fn create_shader_module(
    device: &Device,
    source: &[u32],
) -> Result<vk::ShaderModule, vk::Result> {
    let module_info = vk::ShaderModuleCreateInfo::default().code(source);

    unsafe { device.create_shader_module(&module_info, None) }
}

#[derive(Error, Debug)]
pub enum ShaderBuildError {
    #[error("Failed to read file at provided path \"{provided_path}\" with error: {error}.")]
    InvalidPath {
        provided_path: String,
        error: std::io::Error,
    },

    #[error("SPIRV decoding of stage {stage:?} failed with error: {error}.")]
    SPIRVDecodingFailed {
        stage: vk::ShaderStageFlags,
        error: std::io::Error,
    },

    #[error("Vulkan creation of shader module at stage {stage:?} failed with result: {result}.")]
    ShaderModuleCreationFailed {
        stage: vk::ShaderStageFlags,
        result: vk::Result,
    },

    #[error(
        "SPIRV reflection creation at stage {stage:?} failed with error message: {error_msg}."
    )]
    ReflectionLoadingFailed {
        stage: vk::ShaderStageFlags,
        error_msg: &'static str,
    },

    #[error("Descriptor set layout creation failed with error: {0}.")]
    DSLCreationFailed(#[from] DSLCreationError),
}

#[profiling::all_functions]
impl Shader {
    /// This function expects a valid path for both **SPIR-V compiled** shader files.
    pub fn from_path(
        vertex_path: &Path,
        fragment_path: &Path,
        device: &Device,
    ) -> Result<ThreadSafeRef<Self>, ShaderBuildError> {
        let vertex_spirv =
            fs::read(vertex_path).map_err(|error| ShaderBuildError::InvalidPath {
                provided_path: vertex_path
                    .to_str()
                    .map(|str| str.to_owned())
                    .expect("Failed to parse provided path."),
                error,
            })?;
        let fragment_spirv =
            fs::read(fragment_path).map_err(|error| ShaderBuildError::InvalidPath {
                provided_path: fragment_path
                    .to_str()
                    .map(|str| str.to_owned())
                    .expect("Failed to parse provided path."),
                error,
            })?;

        Self::from_spirv_u8(&vertex_spirv, &fragment_spirv, device)
    }

    /// This function expects **COMPILED SPIR-V**, not higher level languages like GLSL or HSLS source code.
    pub fn from_spirv_u8(
        vertex_spirv: &[u8],
        fragment_spirv: &[u8],
        device: &Device,
    ) -> Result<ThreadSafeRef<Self>, ShaderBuildError> {
        let vertex_u32 =
            ash::util::read_spv(&mut std::io::Cursor::new(vertex_spirv)).map_err(|error| {
                ShaderBuildError::SPIRVDecodingFailed {
                    stage: vk::ShaderStageFlags::VERTEX,
                    error,
                }
            })?;
        let fragment_u32 =
            ash::util::read_spv(&mut std::io::Cursor::new(fragment_spirv)).map_err(|error| {
                ShaderBuildError::SPIRVDecodingFailed {
                    stage: vk::ShaderStageFlags::FRAGMENT,
                    error,
                }
            })?;

        Self::from_spirv_u32(device, &vertex_u32, &fragment_u32)
    }

    /// This function expects **COMPILED SPIR-V**, not higher level languages like GLSL or HSLS source code.
    pub fn from_spirv_u32(
        device: &Device,
        vertex_spirv: &[u32],
        fragment_spirv: &[u32],
    ) -> Result<ThreadSafeRef<Self>, ShaderBuildError> {
        let vertex_module = create_shader_module(device, vertex_spirv).map_err(|result| {
            ShaderBuildError::ShaderModuleCreationFailed {
                stage: vk::ShaderStageFlags::VERTEX,
                result,
            }
        })?;
        let fragment_module = create_shader_module(device, fragment_spirv).map_err(|result| {
            ShaderBuildError::ShaderModuleCreationFailed {
                stage: vk::ShaderStageFlags::FRAGMENT,
                result,
            }
        })?;

        let vertex_reflection_module = spirv_reflect::ShaderModule::load_u32_data(vertex_spirv)
            .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                stage: vk::ShaderStageFlags::VERTEX,
                error_msg,
            })?;
        let vertex_entry_point =
            vertex_reflection_module
                .enumerate_entry_points()
                .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                    stage: vk::ShaderStageFlags::VERTEX,
                    error_msg,
                })?[0]
                .clone();
        let vertex_bindings_reflection = vertex_reflection_module
            .enumerate_descriptor_bindings(Some(vertex_entry_point.name.as_str()))
            .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                stage: vk::ShaderStageFlags::VERTEX,
                error_msg,
            })?;
        let vertex_push_constants = vertex_reflection_module
            .enumerate_push_constant_blocks(Some(vertex_entry_point.name.as_str()))
            .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                stage: vk::ShaderStageFlags::VERTEX,
                error_msg,
            })?;

        let fragment_reflection_module = spirv_reflect::ShaderModule::load_u32_data(fragment_spirv)
            .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                stage: vk::ShaderStageFlags::FRAGMENT,
                error_msg,
            })?;
        let fragment_entry_point = fragment_reflection_module
            .enumerate_entry_points()
            .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                stage: vk::ShaderStageFlags::FRAGMENT,
                error_msg,
            })?[0]
            .clone();
        let fragment_bindings_reflection = fragment_reflection_module
            .enumerate_descriptor_bindings(Some(fragment_entry_point.name.as_str()))
            .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                stage: vk::ShaderStageFlags::FRAGMENT,
                error_msg,
            })?;
        let fragment_push_constants = fragment_reflection_module
            .enumerate_push_constant_blocks(Some(fragment_entry_point.name.as_str()))
            .map_err(|error_msg| ShaderBuildError::ReflectionLoadingFailed {
                stage: vk::ShaderStageFlags::FRAGMENT,
                error_msg,
            })?;

        let level_2_dsl = create_dsl(
            device,
            2,
            &[
                (
                    vertex_bindings_reflection.clone(),
                    vk::ShaderStageFlags::VERTEX,
                ),
                (
                    fragment_bindings_reflection.clone(),
                    vk::ShaderStageFlags::FRAGMENT,
                ),
            ],
        )
        .map_err(ShaderBuildError::DSLCreationFailed)?;
        let level_3_dsl = create_dsl(
            device,
            3,
            &[
                (
                    vertex_bindings_reflection.clone(),
                    vk::ShaderStageFlags::VERTEX,
                ),
                (
                    fragment_bindings_reflection.clone(),
                    vk::ShaderStageFlags::FRAGMENT,
                ),
            ],
        )?;

        let vertex_bindings = vertex_bindings_reflection
            .iter()
            .map(|binding| BindingData {
                set: binding.set,
                slot: binding.binding,
                descriptor_type: binding.descriptor_type,
                size: binding.block.size,
                dim: binding.image.dim,
            })
            .collect::<Vec<_>>();
        let fragment_bindings = fragment_bindings_reflection
            .iter()
            .map(|binding| BindingData {
                set: binding.set,
                slot: binding.binding,
                descriptor_type: binding.descriptor_type,
                size: binding.block.size,
                dim: binding.image.dim,
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
