use std::sync::{Arc, Mutex, MutexGuard};

use ash::vk::{self, CommandBufferResetFlags};
use bevy_ecs::{prelude::Component, system::Resource};
use bytemuck::Zeroable;
use thiserror::Error;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct PodWrapper<T: Copy + 'static>(pub T);
unsafe impl<T: Copy + 'static> Zeroable for PodWrapper<T> {
    fn zeroed() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
unsafe impl<T: Copy + 'static> bytemuck::Pod for PodWrapper<T> {}

#[derive(Debug, Component, Resource)]
pub struct ThreadSafeRef<T>(Arc<Mutex<T>>);

impl<T> ThreadSafeRef<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(Mutex::new(value)))
    }

    pub fn lock(&self) -> MutexGuard<T> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl<T> From<ThreadSafeRef<T>> for Arc<Mutex<T>> {
    fn from(thread_safe_ref: ThreadSafeRef<T>) -> Self {
        thread_safe_ref.0
    }
}

impl<T> Clone for ThreadSafeRef<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Default)]
pub struct CommandUploader {
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
}

#[derive(Error, Debug)]
pub enum CommandUploaderCreationError {
    #[error("Command uploader's vulkan command pool creation failed with result: {0}")]
    VulkanCommandPoolCreationFailed(vk::Result),

    #[error("Command uploader's vulkan fence creation failed with result: {0}")]
    VulkanFenceCreationFailed(vk::Result),

    #[error("Command uploader's vulkan command buffer allocation failed with result: {0}")]
    VulkanCommandBufferAllocationFailed(vk::Result),
}

#[derive(Error, Debug)]
pub enum ImmediateCommandError {
    #[error("Vulkan command buffer begin call failed with result: {0}")]
    VulkanCommandBufferBeginFailed(vk::Result),

    #[error("Vulkan command buffer end call failed with result: {0}")]
    VulkanCommandBufferEndFailed(vk::Result),

    #[error("Vulkan command buffer submission failed with result: {0}")]
    VulkanCommandBufferSubmissionFailed(vk::Result),

    #[error("Vulkan command buffer fence wait failed with result: {0}")]
    VulkanCommandBufferFenceWaitFailed(vk::Result),

    #[error("Vulkan command buffer fence reset failed with result: {0}")]
    VulkanCommandBufferFenceResetFailed(vk::Result),

    #[error("Vulkan command buffer reset failed with result: {0}")]
    VulkanCommandBufferResetFailed(vk::Result),
}

impl CommandUploader {
    pub(crate) fn new(
        device: &ash::Device,
        queue_index: u32,
    ) -> Result<Self, CommandUploaderCreationError> {
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }
            .map_err(|result| {
                CommandUploaderCreationError::VulkanCommandPoolCreationFailed(result)
            })?;

        let fence_info = vk::FenceCreateInfo::default();
        let fence = unsafe { device.create_fence(&fence_info, None) }
            .map_err(CommandUploaderCreationError::VulkanFenceCreationFailed)?;

        let cmd_buffer_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer = unsafe { device.allocate_command_buffers(&cmd_buffer_info) }
            .map_err(|result| {
                CommandUploaderCreationError::VulkanCommandBufferAllocationFailed(result)
            })?
            .swap_remove(0);

        Ok(Self {
            command_pool,
            command_buffer,
            fence,
        })
    }

    pub(crate) fn destroy(self, device: &ash::Device) {
        unsafe {
            device.destroy_fence(self.fence, None);
            device.destroy_command_pool(self.command_pool, None);
        };
    }

    pub fn immediate_command<F>(
        &self,
        device: &ash::Device,
        graphics_queue: vk::Queue,
        function: F,
    ) -> Result<(), ImmediateCommandError>
    where
        F: FnOnce(&vk::CommandBuffer),
    {
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe { device.begin_command_buffer(self.command_buffer, &begin_info) }
            .map_err(ImmediateCommandError::VulkanCommandBufferBeginFailed)?;
        function(&self.command_buffer);
        unsafe { device.end_command_buffer(self.command_buffer) }
            .map_err(ImmediateCommandError::VulkanCommandBufferEndFailed)?;

        let submit_info =
            vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&self.command_buffer));
        unsafe { device.queue_submit(graphics_queue, &[submit_info], self.fence) }
            .map_err(ImmediateCommandError::VulkanCommandBufferSubmissionFailed)?;

        unsafe { device.wait_for_fences(std::slice::from_ref(&self.fence), true, u64::MAX) }
            .map_err(ImmediateCommandError::VulkanCommandBufferFenceWaitFailed)?;
        unsafe { device.reset_fences(std::slice::from_ref(&self.fence)) }
            .map_err(ImmediateCommandError::VulkanCommandBufferFenceResetFailed)?;
        unsafe {
            device.reset_command_buffer(self.command_buffer, CommandBufferResetFlags::default())
        }
        .map_err(ImmediateCommandError::VulkanCommandBufferResetFailed)?;

        Ok(())
    }
}

/// Attempts to name a vulkan object using the `VK_EXT_debug_utils` extension.
///
/// # Panics
/// Panics if a debug messenger is not present in the renderer.
///
/// # Errors
/// This function will return an error if the naming operation fails from the driver.
///
/// # Safety
/// This is safe if and only if name info data is still in scope when this function is called.
#[cfg(debug_assertions)]
pub unsafe fn debug_name_vk_object(
    renderer: &mut crate::renderer::Renderer,
    name_info: &vk::DebugUtilsObjectNameInfoEXT,
) -> ash::prelude::VkResult<()> {
    ash::ext::debug_utils::Device::new(&renderer.instance, &renderer.device)
        .set_debug_utils_object_name(name_info)
}
