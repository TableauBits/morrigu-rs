use std::sync::{Arc, Mutex, MutexGuard};

use ash::vk::{self, CommandBufferResetFlags};
use bevy_ecs::{prelude::Component, system::Resource};

use crate::error::Error;

#[derive(Debug, Component, Resource)]
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

impl CommandUploader {
    pub(crate) fn new(device: &ash::Device, queue_index: u32) -> Result<Self, Error> {
        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .queue_family_index(queue_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        let command_pool = unsafe { device.create_command_pool(&command_pool_info, None) }?;

        let fence_info = vk::FenceCreateInfo::default();
        let fence = unsafe { device.create_fence(&fence_info, None) }?;

        let cmd_buffer_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer =
            unsafe { device.allocate_command_buffers(&cmd_buffer_info) }?.swap_remove(0);

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
    ) -> Result<(), Error>
    where
        F: FnOnce(&vk::CommandBuffer),
    {
        let begin_info = vk::CommandBufferBeginInfo::builder()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe { device.begin_command_buffer(self.command_buffer, &begin_info) }?;
        function(&self.command_buffer);
        unsafe { device.end_command_buffer(self.command_buffer) }?;

        let submit_info =
            vk::SubmitInfo::builder().command_buffers(std::slice::from_ref(&self.command_buffer));
        unsafe { device.queue_submit(graphics_queue, &[*submit_info], self.fence) }?;

        unsafe { device.wait_for_fences(std::slice::from_ref(&self.fence), true, u64::MAX) }?;
        unsafe { device.reset_fences(std::slice::from_ref(&self.fence)) }?;
        unsafe {
            device.reset_command_buffer(self.command_buffer, CommandBufferResetFlags::default())
        }?;

        Ok(())
    }
}
