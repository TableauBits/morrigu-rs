use crate::{
    allocated_types::{AllocatedImage, ImageBuildError, ImageDataUploadError},
    renderer::Renderer,
    utils::{CommandUploader, ImmediateCommandError, ThreadSafeRef},
};

use ash::vk;
use image::{self, EncodableLayout};
use thiserror::Error;

#[non_exhaustive]
#[allow(non_camel_case_types)]
pub enum TextureFormat {
    RGBA8_SRGB,
    RGBA8_UNORM,
}

impl From<TextureFormat> for vk::Format {
    fn from(value: TextureFormat) -> Self {
        match value {
            TextureFormat::RGBA8_SRGB => vk::Format::R8G8B8A8_SRGB,
            TextureFormat::RGBA8_UNORM => vk::Format::R8G8B8A8_UNORM,
        }
    }
}

pub struct TextureBuilder {
    pub format: vk::Format,
    pub layout: vk::ImageLayout,
    pub usage: vk::ImageUsageFlags,
}

#[derive(Error, Debug)]
pub enum TextureBuildError {
    #[error("Base image loading failed with error: {0}.")]
    ImageLoadError(#[from] image::error::ImageError),

    #[error("Creation of texture's underlying image failed with error: {0}.")]
    ImageCreationFailed(#[from] ImageBuildError),

    #[error("Vulkan creation of texture sampler failed with result: {0}.")]
    VulkanSamplerCreationFailed(vk::Result),
}

impl TextureBuilder {
    pub fn new() -> Self {
        Self {
            format: vk::Format::R8G8B8A8_SRGB,
            layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            usage: vk::ImageUsageFlags::empty(),
        }
    }

    pub fn with_format(mut self, format: TextureFormat) -> Self {
        self.format = format.into();

        self
    }

    pub fn with_layout(mut self, layout: vk::ImageLayout) -> Self {
        self.layout = layout;

        self
    }

    pub fn with_usage(mut self, usage: vk::ImageUsageFlags) -> Self {
        self.usage = usage;

        self
    }

    #[profiling::function]
    pub fn build(
        self,
        dimensions: [u32; 2],
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Texture>, TextureBuildError> {
        let pattern = [255, 255, 255, 255, 255, 0, 255, 255];
        let data = pattern
            .iter()
            .cycle()
            .take((4 * dimensions[0] * dimensions[1]).try_into().unwrap())
            .copied()
            .collect::<Vec<_>>();

        self.build_from_data_internal(
            &data,
            dimensions[0],
            dimensions[1],
            &renderer.device,
            renderer.graphics_queue.handle,
            &mut renderer.allocator.as_mut().unwrap().lock(),
            &mut renderer.command_uploader,
        )
    }

    #[profiling::function]
    pub fn build_from_path(
        self,
        path: &std::path::Path,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Texture>, TextureBuildError> {
        let image = image::open(path)?.fliph().into_rgba8();
        let dimensions = image.dimensions();

        let new_texture =
            self.build_from_data(image.as_bytes(), dimensions.0, dimensions.1, renderer)?;
        new_texture.lock().path = Some(path.to_str().unwrap_or("invalid path").to_owned());
        Ok(new_texture)
    }

    #[profiling::function]
    pub fn build_from_data(
        self,
        data: &[u8],
        width: u32,
        height: u32,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Texture>, TextureBuildError> {
        self.build_from_data_internal(
            data,
            width,
            height,
            &renderer.device,
            renderer.graphics_queue.handle,
            &mut renderer.allocator.as_mut().unwrap().lock(),
            &mut renderer.command_uploader,
        )
    }
}

impl TextureBuilder {
    // Used internally to build default texture in the renderer
    pub(crate) fn build_default_internal(
        self,
        device: &ash::Device,
        graphics_queue: vk::Queue,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        command_uploader: &mut CommandUploader,
    ) -> Result<ThreadSafeRef<Texture>, TextureBuildError> {
        self.build_from_data_internal(
            &[
                255, 255, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 255, 255, 255,
            ],
            2,
            2,
            device,
            graphics_queue,
            allocator,
            command_uploader,
        )
    }

    // Internal function only, I can deal with this
    #[allow(clippy::too_many_arguments)]
    fn build_from_data_internal(
        self,
        data: &[u8],
        width: u32,
        height: u32,
        device: &ash::Device,
        graphics_queue: vk::Queue,
        allocator: &mut gpu_allocator::vulkan::Allocator,
        command_uploader: &mut CommandUploader,
    ) -> Result<ThreadSafeRef<Texture>, TextureBuildError> {
        let image = AllocatedImage::builder(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .texture_default(self.format)
        .with_layout(self.layout)
        .with_usage(self.usage)
        .with_data(data.to_vec())
        .build_internal(device, graphics_queue, allocator, command_uploader)?;

        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT);
        let sampler = unsafe { device.create_sampler(&sampler_info, None) }
            .map_err(TextureBuildError::VulkanSamplerCreationFailed)?;

        Ok(ThreadSafeRef::new(Texture {
            image_ref: ThreadSafeRef::new(image),
            sampler,
            path: None,
            dimensions: [width, height],
            format: self.format,
        }))
    }
}

impl Default for TextureBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct Texture {
    pub image_ref: ThreadSafeRef<AllocatedImage>,
    pub sampler: vk::Sampler,

    pub path: Option<String>,
    pub dimensions: [u32; 2],
    format: vk::Format,
}

#[derive(Error, Debug)]
pub enum TextureCloneError {
    #[error("Creation of clone texture's underlying image failed with error: {0}.")]
    ImageCreationFailed(#[from] ImageBuildError),

    #[error("Submission of copy command failed with error: {0}.")]
    CloneCommandFailed(#[from] ImmediateCommandError),

    #[error("Vulkan creation of cloned texture sampler failed with result: {0}.")]
    VulkanSamplerCreationFailed(vk::Result),
}

#[profiling::all_functions]
impl Texture {
    #[profiling::skip]
    pub fn builder() -> TextureBuilder {
        TextureBuilder::default()
    }

    pub fn clone(&self, renderer: &mut Renderer) -> Result<Self, TextureCloneError> {
        let new_image = AllocatedImage::builder(vk::Extent3D {
            width: self.dimensions[0],
            height: self.dimensions[1],
            depth: 1,
        })
        .texture_default(self.format)
        .build_uninitialized(&renderer.device, &mut renderer.allocator())?;

        renderer.immediate_command(|cmd_buffer| {
            let image = self.image_ref.lock();

            let range = vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(image.layer_count);
            let transfer_src_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(vk::AccessFlags::NONE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .image(image.handle)
                .subresource_range(*range);
            let transfer_dst_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(vk::AccessFlags::NONE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .image(new_image.handle)
                .subresource_range(*range);
            unsafe {
                renderer.device.cmd_pipeline_barrier(
                    *cmd_buffer,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[*transfer_src_barrier, *transfer_dst_barrier],
                )
            };

            let copy_region = vk::ImageCopy::builder()
                .src_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: image.layer_count,
                })
                .extent(vk::Extent3D {
                    width: self.dimensions[0],
                    height: self.dimensions[1],
                    depth: 1,
                });
            unsafe {
                let image = self.image_ref.lock();
                renderer.device.cmd_copy_image(
                    *cmd_buffer,
                    image.handle,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    new_image.handle,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    std::slice::from_ref(&copy_region),
                )
            };
            let image = self.image_ref.lock();

            let shader_read_src_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image(image.handle)
                .subresource_range(*range);
            let shader_read_dst_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image(new_image.handle)
                .subresource_range(*range);
            unsafe {
                renderer.device.cmd_pipeline_barrier(
                    *cmd_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[*shader_read_src_barrier, *shader_read_dst_barrier],
                )
            };
        })?;

        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT);
        let sampler = unsafe { renderer.device.create_sampler(&sampler_info, None) }
            .map_err(TextureCloneError::VulkanSamplerCreationFailed)?;

        Ok(Self {
            image_ref: ThreadSafeRef::new(new_image),
            sampler,
            path: self.path.clone(),
            dimensions: self.dimensions,
            format: self.format,
        })
    }

    pub fn upload_data(
        &mut self,
        data: &[u8],
        renderer: &mut Renderer,
    ) -> Result<(), ImageDataUploadError> {
        self.image_ref.lock().upload_data(
            data,
            None,
            &renderer.device,
            renderer.graphics_queue.handle,
            &mut renderer.allocator(),
            &renderer.command_uploader,
        )
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        self.destroy_internal(&renderer.device, &mut renderer.allocator())
    }

    #[profiling::skip]
    pub(crate) fn destroy_internal(
        &mut self,
        device: &ash::Device,
        allocator: &mut gpu_allocator::vulkan::Allocator,
    ) {
        unsafe { device.destroy_sampler(self.sampler, None) };

        self.image_ref.lock().destroy_internal(device, allocator);
    }
}
