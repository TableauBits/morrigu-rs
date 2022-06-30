use ash::vk;
use bytemuck::bytes_of;

use crate::{
    allocated_types::AllocatedBuffer,
    error::Error,
    pipeline_builder::PipelineBuilder,
    renderer::Renderer,
    shader::{binding_type_cast, Shader},
    texture::Texture,
    utils::ThreadSafeRef,
};

use nalgebra_glm as glm;

pub struct VertexInputDescription {
    pub bindings: Vec<vk::VertexInputBindingDescription>,
    pub attributes: Vec<vk::VertexInputAttributeDescription>,
}

pub trait Vertex: std::marker::Sync + std::marker::Send + 'static {
    fn vertex_input_description() -> VertexInputDescription;
}

struct CameraData {
    view_projection_matrix: glm::Mat4,
    world_position: glm::Vec4,
}

pub struct Material<VertexType>
where
    VertexType: Vertex,
{
    descriptor_pool: vk::DescriptorPool,
    uniform_buffers: std::collections::HashMap<u32, AllocatedBuffer>,
    sampled_images: std::collections::HashMap<u32, ThreadSafeRef<Texture>>,

    pub shader_ref: ThreadSafeRef<Shader>,

    pub(crate) descriptor_set: vk::DescriptorSet,
    pub(crate) layout: vk::PipelineLayout,
    pub(crate) pipeline: vk::Pipeline,

    vertex_type_safety: std::marker::PhantomData<VertexType>,
}

pub struct MaterialBuilder {
    pub z_test: bool,
    pub z_write: bool,
}

impl MaterialBuilder {
    pub fn new() -> Self {
        Self {
            z_test: true,
            z_write: true,
        }
    }

    pub fn z_test(mut self, z_test: bool) -> Self {
        self.z_test = z_test;
        self
    }

    pub fn z_write(mut self, z_write: bool) -> Self {
        self.z_write = z_write;
        self
    }

    pub fn build<VertexType>(
        self,
        shader_ref: &ThreadSafeRef<Shader>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Material<VertexType>>, Error>
    where
        VertexType: Vertex,
    {
        let shader_ref = ThreadSafeRef::clone(shader_ref);
        let shader = shader_ref.lock();

        let mut ubo_count = 0;
        let mut sampled_image_count = 0;

        for binding in shader
            .vertex_bindings
            .iter()
            .chain(shader.fragment_bindings.iter())
        {
            if binding.set != 2 {
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
            .set_layouts(std::slice::from_ref(&shader.level_2_dsl));
        let descriptor_set = unsafe {
            renderer
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
        }?[0];

        let mut uniform_buffers = std::collections::HashMap::new();
        let mut sampled_images = std::collections::HashMap::new();

        for binding in shader
            .vertex_bindings
            .iter()
            .chain(shader.fragment_bindings.iter())
        {
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

        let mut pc_shader_stages = vk::ShaderStageFlags::empty();
        if !shader.vertex_push_constants.is_empty() {
            pc_shader_stages |= vk::ShaderStageFlags::VERTEX;
        }
        if !shader.fragment_push_constants.is_empty() {
            pc_shader_stages |= vk::ShaderStageFlags::FRAGMENT;
        }

        let mut pc_ranges = vec![];
        if !pc_shader_stages.is_empty() {
            pc_ranges = vec![vk::PushConstantRange::builder()
                .stage_flags(pc_shader_stages)
                .offset(0)
                .size(std::mem::size_of::<CameraData>().try_into()?)
                .build()]
        }
        let layouts = [
            renderer.descriptors[0].layout,
            renderer.descriptors[1].layout,
            shader.level_2_dsl,
            shader.level_3_dsl,
        ];
        let layout_info = vk::PipelineLayoutCreateInfo::builder()
            .set_layouts(&layouts)
            .push_constant_ranges(&pc_ranges);
        let layout = unsafe { renderer.device.create_pipeline_layout(&layout_info, None) }?;

        let vertex_info = VertexType::vertex_input_description();
        let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&vertex_info.bindings)
            .vertex_attribute_descriptions(&vertex_info.attributes);

        let shader_module_entry_point = std::ffi::CString::new("main").unwrap();
        let vertex_shader_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(shader.vertex_module)
            .name(&shader_module_entry_point);
        let fragment_shader_stage = vk::PipelineShaderStageCreateInfo::builder()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(shader.fragment_module)
            .name(&shader_module_entry_point);

        let input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let rasterizer_state_info = vk::PipelineRasterizationStateCreateInfo::builder()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::CLOCKWISE)
            .line_width(1.0);
        let multisampling_state_info = vk::PipelineMultisampleStateCreateInfo::builder()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0);
        let depth_stencil_state_info = vk::PipelineDepthStencilStateCreateInfo::builder()
            .depth_test_enable(self.z_test)
            .depth_write_enable(self.z_write)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .min_depth_bounds(0.0)
            .max_depth_bounds(1.0);
        let color_blend_attachment_state = vk::PipelineColorBlendAttachmentState::builder()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA);

        let pipeline = PipelineBuilder {
            shader_stages: vec![*vertex_shader_stage, *fragment_shader_stage],
            vertex_input_state_info: *vertex_input_state_info,
            input_assembly_state_info: *input_assembly_state_info,
            rasterizer_state_info: *rasterizer_state_info,
            multisampling_state_info: *multisampling_state_info,
            depth_stencil_state_info: *depth_stencil_state_info,
            color_blend_attachment_state: *color_blend_attachment_state,
            layout,
            cache: None, // @TODO(Ithyx): use pipeline cache plz
        }
        .build(&renderer.device, renderer.primary_render_pass)?;

        drop(shader);

        Ok(ThreadSafeRef::new(Material {
            descriptor_pool,
            uniform_buffers,
            sampled_images,
            shader_ref,
            descriptor_set,
            layout,
            pipeline,
            vertex_type_safety: std::marker::PhantomData,
        }))
    }
}

impl Default for MaterialBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl<VertexType> Material<VertexType>
where
    VertexType: Vertex,
{
    pub fn builder() -> MaterialBuilder {
        MaterialBuilder::new()
    }

    pub fn destroy_owned_textures(&mut self, renderer: &mut Renderer) {
        for texture_ref in self.sampled_images.values() {
            texture_ref.lock().destroy(renderer);
        }
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
        }
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
}
