use std::{io, num::TryFromIntError, str::FromStr};

use ash::vk;

#[derive(Debug)]
pub enum Error {
    VulkanError(vk::Result),
    IOError(io::Error),
    AllocationError(gpu_allocator::AllocationError),
    UnsupportedPlatform(TryFromIntError),
    ImageError(image::ImageError),
    GenericError(String),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IOError(error)
    }
}

impl From<vk::Result> for Error {
    fn from(error: vk::Result) -> Self {
        Error::VulkanError(error)
    }
}

impl From<gpu_allocator::AllocationError> for Error {
    fn from(error: gpu_allocator::AllocationError) -> Self {
        Error::AllocationError(error)
    }
}

impl From<TryFromIntError> for Error {
    fn from(error: TryFromIntError) -> Self {
        Self::UnsupportedPlatform(error)
    }
}

impl From<image::ImageError> for Error {
    fn from(error: image::ImageError) -> Self {
        Self::ImageError(error)
    }
}

impl From<&str> for Error {
    fn from(error: &str) -> Self {
        Error::GenericError(String::from_str(error).expect("Failed to parse error message"))
    }
}
