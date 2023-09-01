use ash::vk;
use bytemuck::bytes_of;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};
use thiserror::Error;

use crate::{
    renderer::Renderer,
    utils::{CommandUploader, ImmediateCommandError},
};

#[derive(Debug, Default)]
pub struct AllocatedBuffer {
    pub handle: vk::Buffer,
    pub(crate) allocation: Option<Allocation>,
    size: u64,
}

#[derive(Error, Debug)]
pub enum BufferDataUploadError {
    #[error("Conversion of data size from usize to u64 failed (check that {0} <= u64::MAX).")]
    SizeConversionFailed(usize),

    #[error(
        "Unable to find this buffer's allocation. This is most likely due to a use after free."
    )]
    UseAfterFree,

    #[error("Invalid data size. The data's size ({data_size}) does not match the buffer's allocation size ({buffer_size}). Please check that T is #[repr(C)].")]
    SizeMismatch { data_size: usize, buffer_size: u64 },

    #[error("Failed to map the memory of this buffer.")]
    MemoryMappingFailed,
}

impl AllocatedBuffer {
    /// This defaults to a uniform buffer usage
    pub fn builder(size: u64) -> AllocatedBufferBuilder {
        AllocatedBufferBuilder::default(size)
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn upload_data<T: bytemuck::Pod>(&mut self, data: T) -> Result<(), BufferDataUploadError> {
        let allocation = self
            .allocation
            .as_mut()
            .ok_or(BufferDataUploadError::UseAfterFree)?;

        if allocation.size()
            < std::mem::size_of::<T>().try_into().map_err(|_| {
                BufferDataUploadError::SizeConversionFailed(std::mem::size_of::<T>())
            })?
        {
            return Err(BufferDataUploadError::SizeMismatch {
                data_size: std::mem::size_of::<T>(),
                buffer_size: allocation.size(),
            });
        }

        let raw_data = bytes_of(&data);
        allocation
            .mapped_slice_mut()
            .ok_or(BufferDataUploadError::MemoryMappingFailed)?[..raw_data.len()]
            .copy_from_slice(raw_data);

        Ok(())
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

#[derive(Error, Debug)]
pub enum BufferBuildError {
    #[error("Vulkan creation of the buffer failed with the result: {0}.")]
    VulkanCreationFailed(vk::Result),

    #[error("allocation of the buffer's memory failed with the error: {0}.")]
    AllocationFailed(#[from] gpu_allocator::AllocationError),

    #[error("Vulkan binding of the buffer's allocation failed with the result: {0}.")]
    VulkanAllocationBindingFailed(vk::Result),
}

#[derive(Error, Debug)]
pub enum BufferBuildWithDataError {
    #[error("Building the buffer failed with: {0}.")]
    BuildFailed(#[from] BufferBuildError),

    #[error("uploading data to the buffer failed with: {0}.")]
    DataUploadFailed(#[from] BufferDataUploadError),
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

    pub fn build(self, renderer: &mut Renderer) -> Result<AllocatedBuffer, BufferBuildError> {
        self.build_internal(&renderer.device, &mut renderer.allocator())
    }

    pub fn build_with_data<T: bytemuck::Pod>(
        self,
        data: T,
        renderer: &mut Renderer,
    ) -> Result<AllocatedBuffer, BufferBuildWithDataError> {
        let mut buffer = self.build(renderer)?;

        buffer.upload_data(data)?;

        Ok(buffer)
    }

    pub(crate) fn build_internal(
        self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<AllocatedBuffer, BufferBuildError> {
        let buffer_info = vk::BufferCreateInfo {
            size: self.size,
            usage: self.usage,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            ..Default::default()
        };

        let handle = unsafe { device.create_buffer(&buffer_info, None) }
            .map_err(BufferBuildError::VulkanCreationFailed)?;

        let memory_req = unsafe { device.get_buffer_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "buffer",
            requirements: memory_req,
            location: self.memory_location,
            linear: true,
            allocation_scheme: AllocationScheme::DedicatedBuffer(handle),
        })?;

        unsafe { device.bind_buffer_memory(handle, allocation.memory(), allocation.offset()) }
            .map_err(BufferBuildError::VulkanAllocationBindingFailed)?;

        Ok(AllocatedBuffer {
            handle,
            allocation: Some(allocation),
            size: self.size,
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
    pub layer_count: u32,
}

#[derive(Error, Debug)]
pub enum ImageDataUploadError {
    #[error("Failed to convert size of data from usize to u64 (check that {0} <= u64::MAX).")]
    SizeConversionFailed(usize),

    #[error("Staging buffer creation failed with error: {0}.")]
    StagingBufferCreationFailed(BufferBuildError),

    #[error(
        "Unable to find the staging buffer's allocation. This is most likely due to a use after free."
    )]
    UseAfterFree,

    #[error("Failed to map the memory of this buffer.")]
    MemoryMappingFailed,

    #[error("The image data copy from the staging buffer failed with the error: {0}.")]
    ImageTransferCommandFailed(#[from] ImmediateCommandError),
}

impl AllocatedImage {
    pub fn upload_data(
        &mut self,
        data: &[u8],
        new_layout: Option<vk::ImageLayout>,
        device: &ash::Device,
        graphics_queue: vk::Queue,
        allocator: &mut Allocator,
        command_uploader: &CommandUploader,
    ) -> Result<(), ImageDataUploadError> {
        let mut staging_buffer = AllocatedBufferBuilder::staging_buffer_default(
            u64::try_from(std::mem::size_of_val(data)).map_err(|_| {
                ImageDataUploadError::SizeConversionFailed(std::mem::size_of_val(data))
            })?,
        )
        .build_internal(device, allocator)
        .map_err(|buffer_build_error| {
            ImageDataUploadError::StagingBufferCreationFailed(buffer_build_error)
        })?;

        let slice = staging_buffer
            .allocation
            .as_mut()
            .ok_or(ImageDataUploadError::UseAfterFree)?
            .mapped_slice_mut()
            .ok_or(ImageDataUploadError::MemoryMappingFailed)?;
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
                    .layer_count(self.layer_count);
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
                            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
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
                        layer_count: self.layer_count,
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
                    .dst_access_mask(vk::AccessFlags::NONE)
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(new_layout.unwrap_or(self.layout))
                    .image(self.handle)
                    .subresource_range(*range);
                unsafe {
                    device.cmd_pipeline_barrier(
                        *cmd_buffer,
                        vk::PipelineStageFlags::TRANSFER,
                        vk::PipelineStageFlags::TOP_OF_PIPE,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        std::slice::from_ref(&shader_read_barrier),
                    )
                };
            },
        )?;

        if let Some(new_layout) = new_layout {
            self.layout = new_layout;
        }

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

    pub layout: vk::ImageLayout,
    pub usage: vk::ImageUsageFlags,

    pub data: Option<Vec<u8>>,
}

#[derive(Error, Debug)]
pub enum ImageBuildError {
    #[error("Vulkan creation of the image failed with the result: {0}.")]
    VulkanCreationFailed(vk::Result),

    #[error("allocation of the allocation's memory failed with the error: {0}.")]
    AllocationFailed(#[from] gpu_allocator::AllocationError),

    #[error("Vulkan binding of the image's allocation failed with the result: {0}.")]
    VulkanAllocationBindingFailed(vk::Result),

    #[error("Vulkan creation of the image's view failed with the result: {0}.")]
    VulkanViewCreationFailed(vk::Result),

    #[error("Upload of the image data failed with the result: {0}.")]
    DataUploadFailed(#[from] ImageDataUploadError),
}

impl<'a> AllocatedImageBuilder<'a> {
    pub fn new(extent: vk::Extent3D) -> Self {
        let image_create_info_builder = vk::ImageCreateInfo::builder().extent(extent);
        let image_view_create_info_builder = vk::ImageViewCreateInfo::builder();

        AllocatedImageBuilder {
            image_create_info_builder,
            image_view_create_info_builder,
            layout: vk::ImageLayout::GENERAL,
            usage: vk::ImageUsageFlags::empty(),
            data: None,
        }
    }

    pub fn with_usage(mut self, usage: vk::ImageUsageFlags) -> Self {
        self.usage = usage;

        self
    }

    pub fn with_data(mut self, data: Vec<u8>) -> Self {
        self.data = Some(data);

        self
    }

    pub fn with_layout(mut self, layout: vk::ImageLayout) -> Self {
        self.layout = layout;

        self
    }

    pub fn texture_default(mut self, format: vk::Format) -> Self {
        self.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        self.image_create_info_builder = self
            .image_create_info_builder
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(self.usage | vk::ImageUsageFlags::SAMPLED)
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

    pub fn cubemap_default(mut self, format: vk::Format) -> Self {
        self.layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;

        self.image_create_info_builder = self
            .image_create_info_builder
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .mip_levels(1)
            .array_layers(6)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(self.usage | vk::ImageUsageFlags::SAMPLED)
            .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        self.image_view_create_info_builder = self
            .image_view_create_info_builder
            .view_type(vk::ImageViewType::CUBE)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 6,
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
            .usage(self.usage | vk::ImageUsageFlags::STORAGE)
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

    pub fn build(self, renderer: &mut Renderer) -> Result<AllocatedImage, ImageBuildError> {
        self.build_internal(
            &renderer.device,
            renderer.graphics_queue.handle,
            &mut renderer.allocator(),
            &renderer.command_uploader,
        )
    }

    pub(crate) fn build_internal(
        mut self,
        device: &ash::Device,
        graphics_queue: vk::Queue,
        allocator: &mut Allocator,
        command_uploader: &CommandUploader,
    ) -> Result<AllocatedImage, ImageBuildError> {
        if self.data.is_some() {
            self.usage |= vk::ImageUsageFlags::TRANSFER_DST;
        }
        self.image_create_info_builder.usage |= self.usage;

        let handle = unsafe { device.create_image(&self.image_create_info_builder, None) }
            .map_err(ImageBuildError::VulkanCreationFailed)?;

        let memory_requirements = unsafe { device.get_image_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "Image allocation",
            requirements: memory_requirements,
            location: gpu_allocator::MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::DedicatedImage(handle),
        })?;
        unsafe { device.bind_image_memory(handle, allocation.memory(), allocation.offset()) }
            .map_err(ImageBuildError::VulkanAllocationBindingFailed)?;

        self.image_view_create_info_builder = self.image_view_create_info_builder.image(handle);
        let view = unsafe { device.create_image_view(&self.image_view_create_info_builder, None) }
            .map_err(ImageBuildError::VulkanViewCreationFailed)?;

        let mut image = AllocatedImage {
            view,
            allocation: Some(allocation),
            handle,
            layout: vk::ImageLayout::UNDEFINED,
            format: self.image_create_info_builder.format,
            extent: self.image_create_info_builder.extent,
            layer_count: self.image_create_info_builder.array_layers,
        };

        let data = match self.data {
            Some(data) => data,
            None => std::iter::repeat(u8::MAX)
                .take(
                    (self.image_create_info_builder.extent.width
                        * self.image_create_info_builder.extent.height
                        * 4)
                    .try_into()
                    .unwrap(),
                )
                .collect(),
        };
        image.upload_data(
            &data,
            Some(self.layout),
            device,
            graphics_queue,
            allocator,
            command_uploader,
        )?;

        Ok(image)
    }

    /// Used internally for texture cloning.
    ///
    /// WARNING: no memory barrier has been applied to the image, meaning it's still in vk::ImageLayout::UNDEFINED
    pub(crate) fn build_uninitialized(
        mut self,
        device: &ash::Device,
        allocator: &mut Allocator,
    ) -> Result<AllocatedImage, ImageBuildError> {
        let handle = unsafe { device.create_image(&self.image_create_info_builder, None) }
            .map_err(ImageBuildError::VulkanViewCreationFailed)?;

        let memory_requirements = unsafe { device.get_image_memory_requirements(handle) };
        let allocation = allocator.allocate(&AllocationCreateDesc {
            name: "Image allocation",
            requirements: memory_requirements,
            location: gpu_allocator::MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::DedicatedImage(handle),
        })?;
        unsafe { device.bind_image_memory(handle, allocation.memory(), allocation.offset()) }
            .map_err(ImageBuildError::VulkanAllocationBindingFailed)?;

        self.image_view_create_info_builder = self.image_view_create_info_builder.image(handle);
        let view = unsafe { device.create_image_view(&self.image_view_create_info_builder, None) }
            .map_err(ImageBuildError::VulkanViewCreationFailed)?;

        Ok(AllocatedImage {
            view,
            allocation: Some(allocation),
            handle,
            layout: vk::ImageLayout::UNDEFINED,
            format: self.image_create_info_builder.format,
            extent: self.image_create_info_builder.extent,
            layer_count: self.image_create_info_builder.array_layers,
        })
    }
}

impl<'a> AllocatedImage {
    pub fn builder(extent: vk::Extent3D) -> AllocatedImageBuilder<'a> {
        AllocatedImageBuilder::new(extent)
    }
}
