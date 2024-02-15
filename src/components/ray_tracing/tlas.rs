use ash::vk;
use bevy_ecs::system::Resource;
use bytemuck::try_cast_slice;
use thiserror::Error;

use crate::{
    allocated_types::{AllocatedBuffer, BufferBuildWithDataError},
    renderer::Renderer,
    utils::{PodWrapper, ThreadSafeRef},
};

#[derive(Error, Debug)]
pub enum TLASBuildError {
    #[error("Failed to cast the blas_list to raw bytes. This is an internal error and should never happen, sorry :( (raw error: {0})")]
    ByteExtractionFailed(bytemuck::PodCastError),

    #[error("The BLAS list results in a size that cannot be converted from usize to u64 (probably too big)")]
    InvalidBLASList,

    #[error("Failed to build the instances staging buffer with error: {0}")]
    StagingBufferBuildError(#[from] BufferBuildWithDataError),
}

// Not tested with multiple TLAS yet, so it stays as a Resource instead of a Component for now
#[derive(Resource)]
pub struct TLAS {}

impl TLAS {
    pub fn new(
        blas_list: &[vk::AccelerationStructureInstanceKHR],
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Self>, TLASBuildError> {
        let data_slice = blas_list
            .iter()
            .map(|blas| PodWrapper(*blas))
            .collect::<Vec<_>>();

        let data: &[u8] =
            try_cast_slice(&data_slice).map_err(TLASBuildError::ByteExtractionFailed)?;

        let instances_buffer = AllocatedBuffer::builder(
            std::mem::size_of_val(data)
                .try_into()
                .map_err(|_| TLASBuildError::InvalidBLASList)?,
        )
        .with_usage(
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
        )
        .build_with_data(data, renderer)?;

        renderer.immediate_command(|cmd_buffer| {});

        Ok(ThreadSafeRef::new(Self {}))
    }

    pub fn update(&mut self) {
        todo!()
    }

    pub fn rebuild(self) -> Self {
        todo!()
    }
}
