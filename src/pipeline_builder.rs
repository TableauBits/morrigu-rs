use ash::vk;
use thiserror::Error;

pub(crate) struct PipelineBuilder<'a> {
    pub(crate) shader_stages: Vec<vk::PipelineShaderStageCreateInfo<'a>>,
    pub(crate) vertex_input_state_info: vk::PipelineVertexInputStateCreateInfo<'a>,  
    pub(crate) input_assembly_state_info: vk::PipelineInputAssemblyStateCreateInfo<'a>,
    pub(crate) rasterizer_state_info: vk::PipelineRasterizationStateCreateInfo<'a>,
    pub(crate) multisampling_state_info: vk::PipelineMultisampleStateCreateInfo<'a>,
    pub(crate) depth_stencil_state_info: vk::PipelineDepthStencilStateCreateInfo<'a>,
    pub(crate) color_blend_attachment_state: vk::PipelineColorBlendAttachmentState,
    pub(crate) layout: vk::PipelineLayout,
    pub(crate) cache: Option<vk::PipelineCache>,
}

#[derive(Error, Debug)]
pub enum PipelineBuildError {
    #[error("Vulkan pipeline creation failed with result: {0}.")]
    VulkanPipelineCreationFailed(#[from] vk::Result),
}

impl PipelineBuilder<'_> {
    pub(crate) fn build(
        self,
        device: &ash::Device,
        render_pass: vk::RenderPass,
    ) -> Result<vk::Pipeline, PipelineBuildError> {
        let viewport_state_info = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let color_blend_info = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(std::slice::from_ref(&self.color_blend_attachment_state));

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&self.shader_stages)
            .vertex_input_state(&self.vertex_input_state_info)
            .input_assembly_state(&self.input_assembly_state_info)
            .viewport_state(&viewport_state_info)
            .rasterization_state(&self.rasterizer_state_info)
            .multisample_state(&self.multisampling_state_info)
            .depth_stencil_state(&self.depth_stencil_state_info)
            .color_blend_state(&color_blend_info)
            .dynamic_state(&dynamic_state_info)
            .layout(self.layout)
            .render_pass(render_pass)
            .subpass(0);

        let result = unsafe {
            device.create_graphics_pipelines(
                self.cache.unwrap_or_default(),
                std::slice::from_ref(&pipeline_info),
                None,
            )
        };

        match result {
            Ok(pipelines) => Ok(pipelines[0]),
            Err((_, result)) => Err(result.into()),
        }
    }
}

pub(crate) struct ComputePipelineBuilder<'a> {
    pub(crate) stage: vk::PipelineShaderStageCreateInfo<'a>,
    pub(crate) layout: vk::PipelineLayout,
    pub(crate) cache: Option<vk::PipelineCache>,
}

impl ComputePipelineBuilder<'_> {
    pub(crate) fn build(self, device: &ash::Device) -> Result<vk::Pipeline, PipelineBuildError> {
        let pipeline_info = vk::ComputePipelineCreateInfo::default()
            .stage(self.stage)
            .layout(self.layout);

        let result = unsafe {
            device.create_compute_pipelines(
                self.cache.unwrap_or_default(),
                std::slice::from_ref(&pipeline_info),
                None,
            )
        };

        match result {
            Ok(pipelines) => Ok(pipelines[0]),
            Err((_, error)) => Err(error.into()),
        }
    }
}
