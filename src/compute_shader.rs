use std::fs;
use std::path::Path;

use crate::allocated_types::AllocatedImage;
use crate::error::Error;
use crate::pipeline_builder::ComputePipelineBuilder;
use crate::renderer::Renderer;
use crate::shader::{binding_type_cast, create_dsl, create_shader_module};
use crate::{
    allocated_types::AllocatedBuffer, shader::BindingData, texture::Texture, utils::ThreadSafeRef,
};

use ash::vk;
use ash::Device;
use bytemuck::bytes_of;
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
    uniform_buffers: std::collections::HashMap<u32, AllocatedBuffer>,
    sampled_images: std::collections::HashMap<u32, ThreadSafeRef<Texture>>,

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
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let source_spirv = fs::read(source_path)?;

        self.build_from_spirv_u8(&source_spirv, renderer)
    }

    pub fn build_from_spirv_u8(
        self,
        source_spirv: &[u8],
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<ComputeShader>, Error> {
        let source_u32 = ash::util::read_spv(&mut std::io::Cursor::new(source_spirv))?;

        self.build_from_spirv_u32(&source_u32, renderer)
    }

    pub fn build_from_spirv_u32(
        self,
        source_spirv: &[u32],
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

        let (ubo_count, sampled_image_count) =
            bindings
                .iter()
                .fold(
                    (0, 0),
                    |(ubo_count, sampled_image_count), binding| match binding_type_cast(
                        binding.descriptor_type,
                    )
                    .unwrap()
                    {
                        vk::DescriptorType::UNIFORM_BUFFER => (ubo_count + 1, sampled_image_count),
                        vk::DescriptorType::COMBINED_IMAGE_SAMPLER => {
                            (ubo_count, sampled_image_count + 1)
                        }
                        _ => (ubo_count, sampled_image_count),
                    },
                );

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
            .set_layouts(std::slice::from_ref(&dsl));
        let descriptor_set = unsafe {
            renderer
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
        }?[0];

        let mut uniform_buffers = std::collections::HashMap::new();
        let mut sampled_images = std::collections::HashMap::new();

        for binding in &bindings {
            if binding.set != 2 {
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
                    let texture_ref = renderer.default_texture_ref.clone();
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

                vk::DescriptorType::STORAGE_IMAGE => {
                    let descriptor_image_info = vk::DescriptorImageInfo::builder()
                        .image_view(storage_image.view)
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

                    let set_write = vk::WriteDescriptorSet::builder()
                        .dst_set(descriptor_set)
                        .dst_binding(binding.slot)
                        .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                        .image_info(std::slice::from_ref(&descriptor_image_info));
                }
                _ => (),
            }
        }

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
            uniform_buffers,
            sampled_images,
            descriptor_set,
            layout,
            pipeline,
        }))
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

    pub fn upload_uniform<T: bytemuck::Pod>(
        &mut self,
        binding_slot: u32,
        data: T,
    ) -> Result<(), Error> {
        let binding_data = self
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
        texture_ref: ThreadSafeRef<Texture>,
        renderer: &mut Renderer,
    ) -> Result<(), Error> {
        if !self.sampled_images.contains_key(&binding_slot) {
            return Err("Invalid binding slot".into());
        };

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
        self.sampled_images.insert(binding_slot, texture_ref);

        Ok(())
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        unsafe {
            for uniform in self.uniform_buffers.values_mut() {
                uniform.destroy(&renderer.device, &mut renderer.allocator());
            }
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
