use std::fs;
use std::path::Path;

use crate::allocated_types::{AllocatedBuffer, AllocatedImage};
use crate::descriptor_resources::{
    create_dsl, generate_descriptors_write_from_bindings, DescriptorResources,
};
use crate::error::Error;
use crate::pipeline_builder::ComputePipelineBuilder;
use crate::renderer::Renderer;
use crate::shader::create_shader_module;
use crate::{shader::BindingData, texture::Texture, utils::ThreadSafeRef};

use ash::vk;

use spirv_reflect::types::ReflectBlockVariable;

pub struct ComputeShaderBuilder {
    pub entry_point: String,
}

pub struct ComputeShader {
    pub(crate) shader_module: vk::ShaderModule,

    pub(crate) dsl: vk::DescriptorSetLayout,

    pub bindings: Vec<BindingData>,
    pub push_constants: Vec<ReflectBlockVariable>,

    descriptor_pool: vk::DescriptorPool,
    descriptor_resources: DescriptorResources,

    pub(crate) descriptor_set: vk::DescriptorSet,
    pub(crate) layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl ComputeShaderBuilder {
    pub fn new() -> Self {
        Self {
            entry_point: String::from("main"),
        }
    }

    pub fn build_from_path(
        self,
        source_path: &Path,
        descriptor_resources: DescriptorResources,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let source_spirv = fs::read(source_path)?;

        self.build_from_spirv_u8(&source_spirv, descriptor_resources, renderer)
    }

    pub fn build_from_spirv_u8(
        self,
        source_spirv: &[u8],
        descriptor_resources: DescriptorResources,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let source_u32 = ash::util::read_spv(&mut std::io::Cursor::new(source_spirv))?;

        self.build_from_spirv_u32(&source_u32, descriptor_resources, renderer)
    }

    pub fn build_from_spirv_u32(
        self,
        source_spirv: &[u32],
        descriptor_resources: DescriptorResources,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let shader_module = create_shader_module(&renderer.device, source_spirv)?;

        let reflection_module = spirv_reflect::ShaderModule::load_u32_data(source_spirv)?;
        let entry_point = reflection_module.enumerate_entry_points()?[0].clone();
        let bindings_reflection =
            reflection_module.enumerate_descriptor_bindings(Some(entry_point.name.as_str()))?;
        let push_constants =
            reflection_module.enumerate_push_constant_blocks(Some(entry_point.name.as_str()))?;

        let dsl = create_dsl(
            &renderer.device,
            0,
            &[(bindings_reflection.clone(), vk::ShaderStageFlags::COMPUTE)],
        )?;

        let bindings = bindings_reflection
            .iter()
            .map(|binding| BindingData {
                set: binding.set,
                slot: binding.binding,
                descriptor_type: binding.descriptor_type,
                size: binding.block.size,
            })
            .collect::<Vec<_>>();

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
            .set_layouts(std::slice::from_ref(&dsl));
        let descriptor_set = unsafe {
            renderer
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
        }?[0];

        let descriptor_writes = generate_descriptors_write_from_bindings(
            &bindings,
            &descriptor_set,
            Some(&[2]),
            &descriptor_resources,
        )?;

        unsafe {
            renderer
                .device
                .update_descriptor_sets(&descriptor_writes, &[])
        };

        let pc_ranges = if push_constants.is_empty() {
            vec![]
        } else {
            vec![vk::PushConstantRange::builder()
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .offset(0)
                .size(push_constants[0].size)
                .build()]
        };

        let dsl_list = [dsl];
        let layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&dsl_list)
            .push_constant_ranges(&pc_ranges);
        let layout = unsafe { renderer.device.create_pipeline_layout(&layout_info, None) }?;

        let shader_module_entry_point = std::ffi::CString::new(self.entry_point).unwrap();
        let shader_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(&shader_module_entry_point);

        let pipeline = ComputePipelineBuilder {
            stage: *shader_stage,
            layout,
            cache: None,
        }
        .build(&renderer.device)?;

        Ok(ThreadSafeRef::new(ComputeShader {
            shader_module,
            dsl,
            bindings,
            push_constants,
            descriptor_pool,
            descriptor_set,
            descriptor_resources,
            layout,
            pipeline,
        }))
    }
}

impl Default for ComputeShaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputeShader {
    pub fn builder() -> ComputeShaderBuilder {
        ComputeShaderBuilder::new()
    }

    pub fn run(&self, renderer: &mut Renderer, group_shape: (u32, u32, u32)) -> Result<(), Error> {
        renderer.immediate_command(|cmd_buffer| unsafe {
            renderer
                .device
                .cmd_dispatch(*cmd_buffer, group_shape.0, group_shape.1, group_shape.2)
        })
    }

    pub fn bind_uniform(
        &mut self,
        binding_slot: u32,
        buffer_ref: ThreadSafeRef<AllocatedBuffer>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<AllocatedBuffer>, Error> {
        let Some(old_buffer) = self.descriptor_resources.uniform_buffers.insert(binding_slot, buffer_ref.clone()) else {
            return Err("Invalid binding slot. Make sure you specify all descriptor resources when initializing this resource.".into());
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

    pub fn bind_storage_image<T: bytemuck::Pod>(
        &mut self,
        binding_slot: u32,
        image_ref: ThreadSafeRef<AllocatedImage>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<AllocatedImage>, Error> {
        let Some(old_image) = self.descriptor_resources.storage_images.insert(binding_slot, image_ref.clone()) else {
            return Err("Invalid binding slot. Make sure you specify all descriptor resources when initializing this resource.".into());
        };

        let image = image_ref.lock();

        let descriptor_image_info = vk::DescriptorImageInfo::builder()
            .image_view(image.view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

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
    ) -> Result<ThreadSafeRef<Texture>, Error> {
        let Some(old_texture) = self.descriptor_resources.sampled_images.insert(binding_slot, texture_ref.clone()) else {
            return Err("Invalid binding slot. Make sure you specify all descriptor resources when initializing this resource.".into());
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
            renderer.device.destroy_pipeline(self.pipeline, None);
            renderer.device.destroy_pipeline_layout(self.layout, None);
            renderer
                .device
                .destroy_descriptor_pool(self.descriptor_pool, None);

            renderer
                .device
                .destroy_descriptor_set_layout(self.dsl, None);
            renderer
                .device
                .destroy_shader_module(self.shader_module, None);
        }
    }
}
