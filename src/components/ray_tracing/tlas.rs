use ash::vk;
use bevy_ecs::system::Resource;
use bytemuck::try_cast_slice;
use thiserror::Error;

use crate::{
    allocated_types::{AllocatedBuffer, BufferBuildError, BufferBuildWithDataError},
    renderer::Renderer,
    utils::{ImmediateCommandError, PodWrapper, ThreadSafeRef},
};

#[derive(Error, Debug)]
pub enum TLASBuildError {
    #[error("Failed to cast the blas_list to raw bytes. This is an internal error and should never happen, sorry :( (raw error: {0})")]
    ByteExtractionFailed(bytemuck::PodCastError),

    #[error("The BLAS list results in a size that cannot be converted from usize to u64 (probably too big)")]
    InvalidBLASList,

    #[error("Failed to build the instances buffer with error: {0}")]
    InstancesBufferBuildError(#[from] BufferBuildWithDataError),

    #[error("Error while running command buffer: {0}")]
    CommandBufferError(#[from] ImmediateCommandError),

    #[error("Failed to build the main buffer with error: {0}")]
    MainBufferBuildError(BufferBuildError),

    #[error("Failed to build the scratch buffer with error: {0}")]
    ScratchBufferBuildError(BufferBuildError),

    #[error("Failed to create the acceleration structure with vk result: {0}")]
    TLASCreationFailed(vk::Result),
}

// Not tested with multiple TLAS yet, so it stays as a Resource instead of a Component for now
#[derive(Resource)]
pub struct TLAS {
    data_buffer: AllocatedBuffer,
    instances_buffer: AllocatedBuffer,
    tlas: vk::AccelerationStructureKHR,
}

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
        .with_name("TLAS instances")
        .with_usage(
            vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                | vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR,
        )
        .build_with_data(data, renderer)?;

        let buffer_address_info =
            vk::BufferDeviceAddressInfo::builder().buffer(instances_buffer.handle);
        let instances_buffer_address = unsafe {
            renderer
                .device
                .get_buffer_device_address(&buffer_address_info)
        };

        let instances_data_info = vk::AccelerationStructureGeometryInstancesDataKHR::builder()
            .data(vk::DeviceOrHostAddressConstKHR {
                device_address: instances_buffer_address,
            });

        let tlas_geometry = vk::AccelerationStructureGeometryKHR::builder()
            .geometry_type(vk::GeometryTypeKHR::INSTANCES)
            .geometry(vk::AccelerationStructureGeometryDataKHR {
                instances: *instances_data_info,
            });

        let build_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
            .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
            .geometries(std::slice::from_ref(&tlas_geometry))
            .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL);

        let acceleration_structure_loader =
            ash::extensions::khr::AccelerationStructure::new(&renderer.instance, &renderer.device);

        let blas_count = blas_list.len() as u32;
        let build_sizes = unsafe {
            acceleration_structure_loader.get_acceleration_structure_build_sizes(
                vk::AccelerationStructureBuildTypeKHR::DEVICE,
                &build_info,
                &[blas_count],
            )
        };

        let data_buffer = AllocatedBuffer::builder(build_sizes.acceleration_structure_size)
            .with_name("TLAS data")
            .with_usage(
                vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR
                    | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            )
            .build(renderer)
            .map_err(TLASBuildError::MainBufferBuildError)?;
        let create_info = vk::AccelerationStructureCreateInfoKHR::builder()
            .ty(vk::AccelerationStructureTypeKHR::TOP_LEVEL)
            .size(build_sizes.acceleration_structure_size)
            .buffer(data_buffer.handle);

        let tlas = unsafe {
            acceleration_structure_loader.create_acceleration_structure(&create_info, None)
        }
        .map_err(TLASBuildError::TLASCreationFailed)?;

        let mut scratch_buffer = AllocatedBuffer::builder(build_sizes.build_scratch_size)
            .with_name("TLAS scratch")
            .with_usage(
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
            )
            .build(renderer)
            .map_err(TLASBuildError::ScratchBufferBuildError)?;
        let buffer_info = vk::BufferDeviceAddressInfo::builder().buffer(scratch_buffer.handle);
        let scratch_address = unsafe { renderer.device.get_buffer_device_address(&buffer_info) };

        let build_info =
            build_info
                .dst_acceleration_structure(tlas)
                .scratch_data(vk::DeviceOrHostAddressKHR {
                    device_address: scratch_address,
                });

        let offset_range =
            vk::AccelerationStructureBuildRangeInfoKHR::builder().primitive_count(blas_count);

        renderer.immediate_command(|cmd_buffer| {
            let barrier = vk::MemoryBarrier::builder()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::ACCELERATION_STRUCTURE_WRITE_KHR);

            unsafe {
                renderer.device.cmd_pipeline_barrier(
                    *cmd_buffer,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::ACCELERATION_STRUCTURE_BUILD_KHR,
                    vk::DependencyFlags::empty(),
                    std::slice::from_ref(&barrier),
                    &[],
                    &[],
                )
            };

            unsafe {
                acceleration_structure_loader.cmd_build_acceleration_structures(
                    *cmd_buffer,
                    std::slice::from_ref(&build_info),
                    &[std::slice::from_ref(&offset_range)],
                )
            };
        })?;

        scratch_buffer.destroy(&renderer.device, &mut renderer.allocator());

        Ok(ThreadSafeRef::new(Self {
            data_buffer,
            instances_buffer,
            tlas,
        }))
    }

    pub fn update(&mut self) {
        todo!()
    }

    pub fn rebuild(self) -> Self {
        todo!()
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        let acceleration_structure_loader =
            ash::extensions::khr::AccelerationStructure::new(&renderer.instance, &renderer.device);
        unsafe {
            acceleration_structure_loader.destroy_acceleration_structure(self.tlas, None);
        }

        self.data_buffer
            .destroy(&renderer.device, &mut renderer.allocator());

        self.instances_buffer
            .destroy(&renderer.device, &mut renderer.allocator());
    }
}
