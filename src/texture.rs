use crate::{allocated_types::AllocatedImage, error::Error, renderer::Renderer};

use ash::vk;
use image::{self, GenericImageView};

pub struct Texture {
    pub image: AllocatedImage,
    pub sampler: vk::Sampler,

    pub path: Option<String>,
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
                .ok_or("Uinitialized allocator")?,
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
        })
    }

    pub fn destroy(self, renderer: &mut Renderer) {
        unsafe { renderer.device.destroy_sampler(self.sampler, None) };

        self.image.destroy(renderer)
    }
}
