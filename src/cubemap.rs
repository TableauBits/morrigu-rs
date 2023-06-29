use ash::vk;

use crate::{allocated_types::AllocatedImage, utils::ThreadSafeRef};

#[derive(Debug)]
pub struct Cubemap {
    pub image_ref: ThreadSafeRef<AllocatedImage>,
    pub sampler: vk::Sampler,

    pub path: Option<String>,
    format: vk::Format,
}
