use ash::vk;

pub struct PipelineBarrier {
    pub src_stage_mask: vk::PipelineStageFlags,
    pub dst_stage_mask: vk::PipelineStageFlags,
    pub dependency_flags: vk::DependencyFlags,
    pub memory_barriers: Vec<vk::MemoryBarrier>,
    pub buffer_memory_barriers: Vec<vk::BufferMemoryBarrier>,
    pub image_memory_barriers: Vec<vk::ImageMemoryBarrier>,
}
