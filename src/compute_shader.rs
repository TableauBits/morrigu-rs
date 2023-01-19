use std::fs;
use std::path::Path;

use crate::error::Error;
use crate::renderer::Renderer;
use crate::shader::{create_shader_module, create_dsl};
use crate::{
    allocated_types::AllocatedBuffer, shader::BindingData, texture::Texture, utils::ThreadSafeRef,
};

use ash::vk;
use ash::Device;
use spirv_reflect::types::ReflectBlockVariable;

pub struct ComputeShaderBuilder {}

pub struct ComputeShader {
    pub(crate) shader_module: vk::ShaderModule,

    pub(crate) level_2_dsl: vk::DescriptorSetLayout,
    pub(crate) level_3_dsl: vk::DescriptorSetLayout,

    pub bindings: Vec<BindingData>,
    pub push_constants: Vec<ReflectBlockVariable>,

    descriptor_pool: vk::DescriptorPool,
    uniform_buffers: std::collections::HashMap<u32, AllocatedBuffer>,
    sampled_images: std::collections::HashMap<u32, ThreadSafeRef<Texture>>,

    pub(crate) descriptor_set: vk::DescriptorSet,
    pub(crate) layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl ComputeShaderBuilder {
    pub fn build_from_path(
        device: &Device,
        source_path: &Path,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let source_spirv = fs::read(source_path)?;

        Self::build_from_spirv_u8(device, &source_spirv, renderer)
    }

    pub fn build_from_spirv_u8(
        device: &Device,
        source_spirv: &[u8],
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let source_u32 = ash::util::read_spv(&mut std::io::Cursor::new(source_spirv))?;

        Self::build_from_spirv_u32(device, &source_u32, renderer)
    }

    pub fn build_from_spirv_u32(
        device: &Device,
        source_spirv: &[u32],
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let shader_module = create_shader_module(device, source_spirv);

        let reflection_module = spirv_reflect::ShaderModule::load_u32_data(source_spirv)?;
        let entry_point = reflection_module.enumerate_entry_points()?[0].clone();
        let bindings_reflection =
            reflection_module.enumerate_descriptor_bindings(Some(entry_point.name.as_str()))?;
        let push_constants =
            reflection_module.enumerate_push_constant_blocks(Some(entry_point.name.as_str()))?;

        let level_2_dsl = create_dsl(
            device,
            2,
            &[(bindings_reflection.clone(), vk::ShaderStageFlags::COMPUTE)],
        );
        let level_3_dsl = create_dsl(
            device,
            3,
            &[(bindings_reflection.clone(), vk::ShaderStageFlags::COMPUTE)],
        );

        let bindings = bindings_reflection
            .iter()
            .map(|binding| BindingData {
                set: binding.set,
                slot: binding.binding,
                descriptor_type: binding.descriptor_type,
                size: binding.block.size,
            })
            .collect::<Vec<_>>();
        
        todo!();
    }
}
