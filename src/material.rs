use ash::vk;
use thiserror::Error;

use crate::{
    allocated_types::{AllocatedBuffer, AllocatedImage},
    descriptor_resources::{
        DescriptorResources, DescriptorSetUpdateError, ResourceBindingError, UniformUpdateError,
    },
    math_types::{Mat4, Vec4},
    pipeline_builder::{PipelineBuildError, PipelineBuilder},
    renderer::Renderer,
    shader::Shader,
    texture::Texture,
    utils::ThreadSafeRef,
};

pub struct VertexInputDescription {
    pub bindings: Vec<vk::VertexInputBindingDescription>,
    pub attributes: Vec<vk::VertexInputAttributeDescription>,
}

pub trait Vertex: Sync + Send + 'static + std::fmt::Debug {
    fn vertex_input_description() -> VertexInputDescription;
    fn position_index() -> usize {
        0
    }
    fn position_offset() -> u32 {
        0
    }
}

#[allow(dead_code)] // We never "read" value from this struct, it's directly uploaded to the GPU without any field access
struct CameraData {
    view_projection_matrix: Mat4,
    world_position: Vec4,
}

#[derive(Debug)]
pub struct Material<VertexType>
where
    VertexType: Vertex,
{
    descriptor_pool: vk::DescriptorPool,
    pub descriptor_resources: DescriptorResources,

    pub shader_ref: ThreadSafeRef<Shader>,

    pub(crate) descriptor_set: vk::DescriptorSet,
    pub(crate) layout: vk::PipelineLayout,
    pub(crate) pipeline: vk::Pipeline,

    vertex_type_safety: std::marker::PhantomData<VertexType>,
}

impl<VertexType> PartialEq for Material<VertexType>
where
    VertexType: Vertex,
{
    fn eq(&self, other: &Self) -> bool {
        self.pipeline == other.pipeline
    }
}

pub use vk::CullModeFlags;

pub struct MaterialBuilder {
    pub z_test: bool,
    pub z_write: bool,
    pub cull_mode: CullModeFlags,
}

#[derive(Error, Debug)]
pub enum MaterialBuildError {
    #[error("Material's vulkan descriptor pool creation failed with status: {0}.")]
    VulkanDescriptorPoolCreationFailed(vk::Result),

    #[error("Material's vulkan descriptor set allocation failed with status: {0}.")]
    VulkanDescriptorSetAllocationFailed(vk::Result),

    #[error("Material's descriptor set update failed with status: {0}.")]
    DescriptorSetUpdateFailed(#[from] DescriptorSetUpdateError),

    #[error(
        "No push constants were detected in the shader, but they are needed for the program data."
    )]
    InvalidPushConstantSize,

    #[error("Material's vulkan pipeline layout creation failed with status: {0}.")]
    VulkanPipelineLayoutCreationFailed(vk::Result),

    #[error("Material's creation failed with error: {0}.")]
    PipelineCreationFailed(#[from] PipelineBuildError),
}

impl MaterialBuilder {
    pub fn new() -> Self {
        Self {
            z_test: true,
            z_write: true,
            cull_mode: CullModeFlags::BACK,
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

    pub fn cull_mode(mut self, cull_mode: CullModeFlags) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    #[profiling::function]
    pub fn build<VertexType>(
        self,
        shader_ref: &ThreadSafeRef<Shader>,
        descriptor_resources: DescriptorResources,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Material<VertexType>>, MaterialBuildError>
    where
        VertexType: Vertex,
    {
        let shader_ref = ThreadSafeRef::clone(shader_ref);
        let shader = shader_ref.lock();

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
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .max_sets(1)
            .pool_sizes(&pool_sizes);
        let descriptor_pool = unsafe { renderer.device.create_descriptor_pool(&pool_info, None) }
            .map_err(MaterialBuildError::VulkanDescriptorPoolCreationFailed)?;

        let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(std::slice::from_ref(&shader.level_2_dsl));
        let descriptor_set = unsafe {
            renderer
                .device
                .allocate_descriptor_sets(&descriptor_set_alloc_info)
        }
        .map_err(MaterialBuildError::VulkanDescriptorSetAllocationFailed)?[0];

        let mut merged_bindings = shader.vertex_bindings.clone();
        merged_bindings.extend(&shader.fragment_bindings);
        descriptor_resources.update_descriptors_set_from_bindings(
            &merged_bindings,
            &descriptor_set,
            Some(&[2]),
            renderer,
        )?;

        let mut pc_shader_stages = vk::ShaderStageFlags::empty();
        let mut size = None;
        if !shader.vertex_push_constants.is_empty() {
            pc_shader_stages |= vk::ShaderStageFlags::VERTEX;
            size = Some(shader.vertex_push_constants[0].size);
        }
        if !shader.fragment_push_constants.is_empty() {
            pc_shader_stages |= vk::ShaderStageFlags::FRAGMENT;
            size = Some(shader.fragment_push_constants[0].size);
        }

        let mut pc_ranges = vec![];
        if !pc_shader_stages.is_empty() {
            pc_ranges = vec![vk::PushConstantRange::default()
                .stage_flags(pc_shader_stages)
                .offset(0)
                .size(size.ok_or(MaterialBuildError::InvalidPushConstantSize)?)]
        }
        let layouts = [
            renderer.descriptors[0].layout,
            renderer.descriptors[1].layout,
            shader.level_2_dsl,
            shader.level_3_dsl,
        ];
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts)
            .push_constant_ranges(&pc_ranges);
        let layout = unsafe { renderer.device.create_pipeline_layout(&layout_info, None) }
            .map_err(MaterialBuildError::VulkanPipelineLayoutCreationFailed)?;

        let vertex_info = VertexType::vertex_input_description();
        let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&vertex_info.bindings)
            .vertex_attribute_descriptions(&vertex_info.attributes);

        let shader_module_entry_point = std::ffi::CString::new("main").unwrap();
        let vertex_shader_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(shader.vertex_module)
            .name(&shader_module_entry_point);
        let fragment_shader_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(shader.fragment_module)
            .name(&shader_module_entry_point);

        let input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let rasterizer_state_info = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(self.cull_mode)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);
        let multisampling_state_info = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .min_sample_shading(1.0);
        let depth_stencil_state_info = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(self.z_test)
            .depth_write_enable(self.z_write)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .min_depth_bounds(0.0)
            .max_depth_bounds(1.0);
        let color_blend_attachment_state = vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .color_write_mask(vk::ColorComponentFlags::RGBA);

        let pipeline = PipelineBuilder {
            shader_stages: vec![vertex_shader_stage, fragment_shader_stage],
            vertex_input_state_info,
            input_assembly_state_info,
            rasterizer_state_info,
            multisampling_state_info,
            depth_stencil_state_info,
            color_blend_attachment_state,
            layout,
            cache: None, // @TODO(Ithyx): use pipeline cache plz
        }
        .build(&renderer.device, renderer.primary_render_pass)?;

        drop(shader);

        Ok(ThreadSafeRef::new(Material {
            descriptor_pool,
            descriptor_resources,
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

#[profiling::all_functions]
impl<VertexType> Material<VertexType>
where
    VertexType: Vertex,
{
    #[profiling::skip]
    pub fn builder() -> MaterialBuilder {
        MaterialBuilder::new()
    }

    pub fn bind_uniform<T: bytemuck::Pod>(
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
                set: 2,
            });
        };

        let buffer = buffer_ref.lock();

        let descriptor_buffer_info = vk::DescriptorBufferInfo::default()
            .buffer(buffer.handle)
            .offset(0)
            .range(buffer.allocation.as_ref().unwrap().size());

        let set_write = vk::WriteDescriptorSet::default()
            .dst_set(self.descriptor_set)
            .dst_binding(binding_slot)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .buffer_info(std::slice::from_ref(&descriptor_buffer_info));

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
                set: 2,
            })?
            .lock()
            .upload_pod(data)
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
                set: 2,
            });
        };

        let image = image_ref.lock();

        let descriptor_image_info = vk::DescriptorImageInfo::default()
            .image_view(image.view)
            .image_layout(vk::ImageLayout::GENERAL);

        let set_write = vk::WriteDescriptorSet::default()
            .dst_set(self.descriptor_set)
            .dst_binding(binding_slot)
            .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
            .image_info(std::slice::from_ref(&descriptor_image_info));

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
                set: 2,
            });
        };

        let texture = texture_ref.lock();

        let descriptor_image_info = vk::DescriptorImageInfo::default()
            .sampler(texture.sampler)
            .image_view(texture.image_ref.lock().view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

        let set_write = vk::WriteDescriptorSet::default()
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
        }
    }
}
