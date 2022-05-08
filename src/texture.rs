use crate::{allocated_types::AllocatedImage, error::Error, renderer::Renderer};

use ash::vk;
use image::{self, GenericImageView};

pub struct Texture {
    pub image: AllocatedImage,
    pub sampler: vk::Sampler,

    pub path: Option<String>,
    pub dimensions: [u32; 2],
}

impl Texture {
    pub fn default(renderer: &mut Renderer) -> Result<Self, Error> {
        Self::from_data(
            &[
                255, 255, 255, 255, 255, 0, 255, 255, 255, 0, 255, 255, 255, 255, 255, 255,
            ],
            2,
            2,
            renderer,
        )
    }

    pub fn from_path(
        path: &std::path::Path,
        renderer: &mut Renderer, // device: &ash::Device,
                                 // graphics_queue: vk::Queue,
                                 // allocator: &mut gpu_allocator::vulkan::Allocator,
                                 // command_uploader: &CommandUploader
    ) -> Result<Self, Error> {
        let image = image::open(path)?.fliph();
        let dimensions = image.dimensions();

        let mut new_texture =
            Self::from_data(image.as_bytes(), dimensions.0, dimensions.1, renderer)?;
        new_texture.path = Some(path.to_str().unwrap_or("invalid path").to_owned());
        Ok(new_texture)
    }

    pub fn from_data(
        data: &[u8],
        width: u32,
        height: u32,
        renderer: &mut Renderer,
        // device: &ash::Device,
        // graphics_queue: vk::Queue,
        // allocator: &mut gpu_allocator::vulkan::Allocator,
        // command_uploader: &CommandUploader,
    ) -> Result<Self, Error> {
        let device = &renderer.device;

        let image = AllocatedImage::builder(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .texture_default()
        .build(
            data,
            device,
            renderer.graphics_queue.handle,
            renderer
                .allocator
                .as_mut()
                .ok_or("Unintialized allocator")?,
            &renderer.command_uploader,
        )?;

        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE);
        let sampler = unsafe { device.create_sampler(&sampler_info, None) }?;

        Ok(Self {
            image,
            sampler,
            path: None,
            dimensions: [width, height],
        })
    }

    pub fn clone(&self, renderer: &mut Renderer) -> Result<Self, Error> {
        let new_image = AllocatedImage::builder(vk::Extent3D {
            width: self.dimensions[0],
            height: self.dimensions[1],
            depth: 1,
        })
        .texture_default()
        .build_uninitialized(
            &renderer.device,
            renderer
                .allocator
                .as_mut()
                .ok_or("Unintialized allocator")?,
        )?;

        renderer.immediate_command(|cmd_buffer| {
            let range = vk::ImageSubresourceRange::builder()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1);
            let transfer_src_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(vk::AccessFlags::NONE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .image(self.image.handle)
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
                    layer_count: 1,
                })
                .extent(vk::Extent3D {
                    width: self.dimensions[0],
                    height: self.dimensions[1],
                    depth: 1,
                });
            unsafe {
                renderer.device.cmd_copy_image(
                    *cmd_buffer,
                    self.image.handle,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    new_image.handle,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    std::slice::from_ref(&copy_region),
                )
            };

            let shader_read_src_barrier = vk::ImageMemoryBarrier::builder()
                .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image(self.image.handle)
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
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE);
        let sampler = unsafe { renderer.device.create_sampler(&sampler_info, None) }?;

        Ok(Self {
            image: new_image,
            sampler,
            path: self.path.clone(),
            dimensions: self.dimensions,
        })
    }

    pub fn destroy(self, renderer: &mut Renderer) {
        unsafe { renderer.device.destroy_sampler(self.sampler, None) };

        self.image.destroy(renderer)
    }
}
