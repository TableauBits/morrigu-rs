use crate::{
    allocated_types::{AllocatedImage, ImageBuildError},
    renderer::Renderer,
    texture::TextureFormat,
    utils::ThreadSafeRef,
};

use ash::vk;
use image::{self, EncodableLayout};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CubemapBuildError {
    #[error("Base image loading failed with error: {0}.")]
    ImageLoadError(#[from] image::error::ImageError),

    #[error("Creation of texture's underlying image failed with error: {0}.")]
    ImageCreationFailed(#[from] ImageBuildError),

    #[error("Vulkan creation of texture sampler failed with result: {0}.")]
    VulkanSamplerCreationFailed(vk::Result),
}

#[derive(Debug)]
pub struct Cubemap {
    pub image_ref: ThreadSafeRef<AllocatedImage>,
    pub sampler: vk::Sampler,

    pub path: Option<String>,
}

#[profiling::all_functions]
impl Cubemap {
    pub fn build_from_folder(
        folder_path: &str,
        extension: &str,
        format: TextureFormat,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Cubemap>, CubemapBuildError> {
        let front_path: std::path::PathBuf = [folder_path, format!("front.{extension}").as_str()]
            .iter()
            .collect();
        let back_path: std::path::PathBuf = [folder_path, format!("back.{extension}").as_str()]
            .iter()
            .collect();
        let top_path: std::path::PathBuf = [folder_path, format!("top.{extension}").as_str()]
            .iter()
            .collect();
        let bottom_path: std::path::PathBuf = [folder_path, format!("bottom.{extension}").as_str()]
            .iter()
            .collect();
        let right_path: std::path::PathBuf = [folder_path, format!("right.{extension}").as_str()]
            .iter()
            .collect();
        let left_path: std::path::PathBuf = [folder_path, format!("left.{extension}").as_str()]
            .iter()
            .collect();

        let front_image = image::open(front_path)?.fliph().into_rgba8();
        let back_image = image::open(back_path)?.fliph().into_rgba8();
        let top_image = image::open(top_path)?.fliph().into_rgba8();
        let bottom_image = image::open(bottom_path)?.fliph().into_rgba8();
        let right_image = image::open(right_path)?.fliph().into_rgba8();
        let left_image = image::open(left_path)?.fliph().into_rgba8();

        let initial_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let format: vk::Format = format.into();
        let (width, height) = front_image.dimensions();
        let data = [
            front_image.as_bytes(),
            back_image.as_bytes(),
            top_image.as_bytes(),
            bottom_image.as_bytes(),
            right_image.as_bytes(),
            left_image.as_bytes(),
        ]
        .concat();

        let final_image = AllocatedImage::builder(vk::Extent3D {
            width,
            height,
            depth: 1,
        })
        .cubemap_default(format)
        .with_layout(initial_layout)
        .with_data(data)
        .build(renderer)?;

        let sampler_info = vk::SamplerCreateInfo::builder()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT);
        let sampler = unsafe { renderer.device.create_sampler(&sampler_info, None) }
            .map_err(CubemapBuildError::VulkanSamplerCreationFailed)?;

        Ok(ThreadSafeRef::new(Cubemap {
            image_ref: ThreadSafeRef::new(final_image),
            sampler,
            path: Some(folder_path.to_owned()),
        }))
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        unsafe { renderer.device.destroy_sampler(self.sampler, None) };

        self.image_ref.lock().destroy(renderer);
    }
}
