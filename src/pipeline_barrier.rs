use ash::vk;

pub struct PipelineBarrier<'a> {
    pub src_stage_mask: vk::PipelineStageFlags,
    pub dst_stage_mask: vk::PipelineStageFlags,
    pub dependency_flags: vk::DependencyFlags,
    pub memory_barriers: Vec<vk::MemoryBarrier<'a>>,
    pub buffer_memory_barriers: Vec<vk::BufferMemoryBarrier<'a>>,
    pub image_memory_barriers: Vec<vk::ImageMemoryBarrier<'a>>,
}
