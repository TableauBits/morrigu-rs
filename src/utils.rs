use std::sync::{Arc, Mutex, MutexGuard};

use ash::vk::{self, CommandPoolResetFlags};
use bevy_ecs::prelude::Component;

use crate::error::Error;

#[derive(Component)]
pub struct ThreadSafeRef<T>(Arc<Mutex<T>>);

impl<T> ThreadSafeRef<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(Mutex::new(value)))
    }

    pub fn lock(&self) -> MutexGuard<T> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
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
    fence: vk::Fence,
}

impl CommandUploader {
    pub(crate) fn new(device: &ash::Device, queue_index: u32) -> Result<Self, Error> {
        let command_pool_info =
            vk::CommandPoolCreateInfo::builder().queue_family_index(queue_index);
        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }?;

        let fence_info = vk::FenceCreateInfo::default();
        let fence = unsafe { device.create_fence(&fence_info, None) }?;

        Ok(Self {
            command_pool,
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
    ) -> Result<(), Error>
    where
        F: FnOnce(&vk::CommandBuffer),
    {
        // Allocate command buffer
        let cmd_buffer_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(self.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let cmd_buffer =
            unsafe { device.allocate_command_buffers(&cmd_buffer_info) }?.swap_remove(0);

        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe { device.begin_command_buffer(cmd_buffer, &begin_info) }?;
        function(&cmd_buffer);
        unsafe { device.end_command_buffer(cmd_buffer) }?;

        let submit_info =
            vk::SubmitInfo::builder().command_buffers(std::slice::from_ref(&cmd_buffer));
        unsafe { device.queue_submit(graphics_queue, &[*submit_info], self.fence) }?;

        unsafe { device.wait_for_fences(std::slice::from_ref(&self.fence), true, u64::MAX) }?;
        unsafe { device.reset_fences(std::slice::from_ref(&self.fence)) }?;
        unsafe { device.reset_command_pool(self.command_pool, CommandPoolResetFlags::default()) }?;

        Ok(())
    }
}
