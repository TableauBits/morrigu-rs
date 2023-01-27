use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, Allocator};

use crate::{error::Error, renderer::Renderer, utils::CommandUploader};

#[derive(Debug, Default)]
pub struct AllocatedBuffer {
    pub handle: vk::Buffer,
    pub(crate) allocation: Option<Allocation>,
}

impl AllocatedBuffer {
    /// This defaults to a uniform buffer usage
    pub fn builder(size: u64) -> AllocatedBufferBuilder {
        AllocatedBufferBuilder::default(size)
    }

    pub fn destroy(&mut self, device: &ash::Device, allocator: &mut Allocator) {
        if let Some(allocation) = self.allocation.take() {
            allocator
                .free(allocation)
                .expect("Failed to free buffer memory");
            unsafe { device.destroy_buffer(self.handle, None) };
        }
    }
}

pub struct AllocatedBufferBuilder {
    pub size: u64,
    pub usage: vk::BufferUsageFlags,
    pub memory_location: gpu_allocator::MemoryLocation,
}

impl AllocatedBufferBuilder {
    /// This is equivalent to `uniform_buffer_default`
    pub fn default(size: u64) -> Self {
        Self::uniform_buffer_default(size)
    }

    pub fn uniform_buffer_default(size: u64) -> Self {
        Self {
            size,
            usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
            memory_location: gpu_allocator::MemoryLocation::CpuToGpu,
        }
    }

    pub fn staging_buffer_default(size: u64) -> Self {
        Self {
            size,
            usage: vk::BufferUsageFlags::TRANSFER_SRC,
            memory_location: gpu_allocator::MemoryLocation::CpuToGpu,
        }
    }

    pub fn with_usage(mut self, usage: vk::BufferUsageFlags) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_memory_location(mut self, memory_location: gpu_allocator::MemoryLocation) -> Self {
        self.memory_location = memory_location;
        self
    }

    pub fn build(
        self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<AllocatedBuffer, Error> {
        let buffer_info = vk::BufferCreateInfo {
            size: self.size,
            usage: self.usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let handle = unsafe { device.create_buffer(&buffer_info, None) }?;

        let memory_req = unsafe { device.get_buffer_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "buffer",
            requirements: memory_req,
            location: self.memory_location,
            linear: true,
        })?;

        unsafe { device.bind_buffer_memory(handle, allocation.memory(), allocation.offset()) }?;

        Ok(AllocatedBuffer {
            handle,
            allocation: Some(allocation),
        })
    }
}

#[derive(Debug, Default)]
pub struct AllocatedImage {
    pub view: vk::ImageView,
    pub allocation: Option<Allocation>,
    pub handle: vk::Image,

    pub layout: vk::ImageLayout,
    pub format: vk::Format,
    pub extent: vk::Extent3D,
}

impl AllocatedImage {
    pub fn upload_data(
        &mut self,
        data: &[u8],
        device: &ash::Device,
        graphics_queue: vk::Queue,
        allocator: &mut Allocator,
        command_uploader: &CommandUploader,
    ) -> Result<(), Error> {
        let mut staging_buffer = AllocatedBufferBuilder::staging_buffer_default(u64::try_from(
            data.len() * std::mem::size_of::<u8>(), // Multiplication is redundant, but just in case :3 (technically a byte is not necessarily 8 bits)
        )?)
        .build(device, allocator)?;

        let slice = staging_buffer
            .allocation
            .as_mut()
            .ok_or("use after free")?
            .mapped_slice_mut()
            .ok_or_else(|| {
                gpu_allocator::AllocationError::FailedToMap("Failed to map memory".to_owned())
            })?;
        // copy_from_slice panics if slices are of diffrent lengths, so we have to set a limit
        // just in case the allocation decides to allocate more
        slice[..data.len()].copy_from_slice(data);

        command_uploader.immediate_command(
            device,
            graphics_queue,
            |cmd_buffer: &vk::CommandBuffer| {
                let range = vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1);
                if self.layout != vk::ImageLayout::TRANSFER_DST_OPTIMAL {
                    let transfer_dst_barrier = vk::ImageMemoryBarrier::builder()
                        .src_access_mask(vk::AccessFlags::NONE)
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .old_layout(self.layout)
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .image(self.handle)
                        .subresource_range(*range);
                    unsafe {
                        device.cmd_pipeline_barrier(
                            *cmd_buffer,
                            vk::PipelineStageFlags::TOP_OF_PIPE,
                            vk::PipelineStageFlags::TRANSFER,
                            vk::DependencyFlags::empty(),
                            &[],
                            &[],
                            std::slice::from_ref(&transfer_dst_barrier),
                        )
                    };
                }

                let copy_region = vk::BufferImageCopy::builder()
                    .image_subresource(vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image_extent(self.extent);
                unsafe {
                    device.cmd_copy_buffer_to_image(
                        *cmd_buffer,
                        staging_buffer.handle,
                        self.handle,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        std::slice::from_ref(&copy_region),
                    )
                };

                let shader_read_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image(self.handle)
                    .subresource_range(*range);
                unsafe {
                    device.cmd_pipeline_barrier(
                        *cmd_buffer,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::PipelineStageFlags::FRAGMENT_SHADER,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        std::slice::from_ref(&shader_read_barrier),
                    )
                };
            },
        )?;

        self.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        staging_buffer.destroy(device, allocator);

        Ok(())
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        self.destroy_internal(&renderer.device, &mut renderer.allocator())
    }

    pub(crate) fn destroy_internal(
        &mut self,
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
    ) {
        if let Some(allocation) = self.allocation.take() {
            unsafe { device.destroy_image_view(self.view, None) };
            allocator
                .free(allocation)
                .expect("Failed to free image memory");
            unsafe { device.destroy_image(self.handle, None) };
        }
    }
}

pub struct AllocatedImageBuilder<'a> {
    pub image_create_info_builder: vk::ImageCreateInfoBuilder<'a>,
    pub image_view_create_info_builder: vk::ImageViewCreateInfoBuilder<'a>,
}

impl<'a> AllocatedImageBuilder<'a> {
    pub fn new(extent: vk::Extent3D) -> Self {
        let image_create_info_builder = vk::ImageCreateInfo::builder().extent(extent);
        let image_view_create_info_builder = vk::ImageViewCreateInfo::builder();

        AllocatedImageBuilder {
            image_create_info_builder,
            image_view_create_info_builder,
        }
    }

    pub fn texture_default(mut self, format: vk::Format) -> Self {
        self.image_create_info_builder = self
            .image_create_info_builder
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(
                vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::SAMPLED,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        self.image_view_create_info_builder = self
            .image_view_create_info_builder
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        self
    }

    pub fn storage_image_default(mut self, format: vk::Format) -> Self {
        self.image_create_info_builder = self
            .image_create_info_builder
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(
                vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::STORAGE,
            )
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        self.image_view_create_info_builder = self
            .image_view_create_info_builder
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        self
    }

    pub fn build(
        mut self,
        data: &[u8],
        device: &ash::Device,
        graphics_queue: vk::Queue,
        allocator: &mut Allocator,
        command_uploader: &CommandUploader,
    ) -> Result<AllocatedImage, Error> {
        let handle = unsafe { device.create_image(&self.image_create_info_builder, None) }
            .expect("Failed to create image");

        let memory_requirements = unsafe { device.get_image_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "Image allocation",
            requirements: memory_requirements,
            location: gpu_allocator::MemoryLocation::GpuOnly,
            linear: false,
        })?;
        unsafe { device.bind_image_memory(handle, allocation.memory(), allocation.offset()) }?;

        self.image_view_create_info_builder = self.image_view_create_info_builder.image(handle);
        let view = unsafe { device.create_image_view(&self.image_view_create_info_builder, None) }?;

        let mut image = AllocatedImage {
            view,
            allocation: Some(allocation),
            handle,
            layout: vk::ImageLayout::UNDEFINED,
            format: self.image_create_info_builder.format,
            extent: self.image_create_info_builder.extent,
        };

        image.upload_data(data, device, graphics_queue, allocator, command_uploader)?;

        Ok(image)
    }

    /// Used internally for texture cloning.
    ///
    /// WARNING: no memory barrier has been applied to the image, meaning it's still in vk::ImageLayout::UNDEFINED
    pub(crate) fn build_uninitialized(
        mut self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<AllocatedImage, Error> {
        let handle = unsafe { device.create_image(&self.image_create_info_builder, None) }
            .expect("Failed to create image");

        let memory_requirements = unsafe { device.get_image_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "Image allocation",
            requirements: memory_requirements,
            location: gpu_allocator::MemoryLocation::GpuOnly,
            linear: false,
        })?;
        unsafe { device.bind_image_memory(handle, allocation.memory(), allocation.offset()) }?;

        self.image_view_create_info_builder = self.image_view_create_info_builder.image(handle);
        let view = unsafe { device.create_image_view(&self.image_view_create_info_builder, None) }?;

        Ok(AllocatedImage {
            view,
            allocation: Some(allocation),
            handle,
            layout: vk::ImageLayout::UNDEFINED,
            format: self.image_create_info_builder.format,
            extent: self.image_create_info_builder.extent,
        })
    }
}

impl<'a> AllocatedImage {
    pub fn builder(extent: vk::Extent3D) -> AllocatedImageBuilder<'a> {
        AllocatedImageBuilder::new(extent)
    }
}
