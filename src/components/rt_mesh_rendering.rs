use ash::vk;
use bevy_ecs::prelude::Component;
use thiserror::Error;

use crate::{
    allocated_types::{AllocatedBuffer, BufferBuildError},
    material::Vertex,
    mesh::Mesh,
    renderer::Renderer,
    utils::{ImmediateCommandError, ThreadSafeRef},
};

#[derive(Debug, Component)]
pub struct RTMeshRendering<VertexType: Vertex> {
    pub mesh_ref: ThreadSafeRef<Mesh<VertexType>>,
}

#[derive(Error, Debug)]
pub enum RTMeshRenderingBuildError {
    #[error("Invalid mesh, this component requires meshes to have an index buffer")]
    NonIndexedMesh,

    #[error("Size of vertex is too big (how did you manage to make size_of not fit in a u64 ?!)")]
    InvalidVertexSize,

    #[error("Too many vertices in the mesh, mesh.vertices.len() - 1 must fit in a u32")]
    TooManyVertices,

    #[error("Too many indices in the mesh, mesh.indices.len() / 3 must fit in a u32")]
    TooManyIndices,

    #[error("Failed to build buffer with error: {0}.")]
    BufferBuildError(#[from] BufferBuildError),

    #[error("Failed to create acceleration structure with error: {0}")]
    AccelStructureCreationFailed(vk::Result),

    #[error("BLAS building failed with error: {0}")]
    BLASBuildingFailed(ImmediateCommandError),
}

impl<VertexType: Vertex> RTMeshRendering<VertexType> {
    pub fn new(
        mesh_ref: ThreadSafeRef<Mesh<VertexType>>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Self>, RTMeshRenderingBuildError> {
        {
            let mesh = mesh_ref.lock();

            let buffer_info =
                vk::BufferDeviceAddressInfo::builder().buffer(mesh.vertex_buffer.handle);
            let vertex_address = unsafe { renderer.device.get_buffer_device_address(&buffer_info) };

            let buffer_info = buffer_info.buffer(
                mesh.index_buffer
                    .as_ref()
                    .ok_or(RTMeshRenderingBuildError::NonIndexedMesh)?
                    .handle,
            );
            let index_address = unsafe { renderer.device.get_buffer_device_address(&buffer_info) };

            let triangle_data = vk::AccelerationStructureGeometryTrianglesDataKHR::builder()
                .vertex_format(
                    VertexType::vertex_input_description().attributes[VertexType::position_index()]
                        .format,
                )
                .vertex_data(vk::DeviceOrHostAddressConstKHR {
                    device_address: vertex_address,
                })
                .vertex_stride(
                    std::mem::size_of::<VertexType>()
                        .try_into()
                        .map_err(|_| RTMeshRenderingBuildError::InvalidVertexSize)?,
                )
                .index_type(vk::IndexType::UINT32)
                .index_data(vk::DeviceOrHostAddressConstKHR {
                    device_address: index_address,
                })
                .max_vertex(
                    (mesh.vertices.len() - 1)
                        .try_into()
                        .map_err(|_| RTMeshRenderingBuildError::TooManyVertices)?,
                );

            let geometry = vk::AccelerationStructureGeometryKHR::builder()
                .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
                .flags(vk::GeometryFlagsKHR::OPAQUE)
                .geometry(vk::AccelerationStructureGeometryDataKHR {
                    triangles: *triangle_data,
                });
            let geometry_info = vk::AccelerationStructureBuildGeometryInfoKHR::builder()
                .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                .mode(vk::BuildAccelerationStructureModeKHR::BUILD)
                .flags(vk::BuildAccelerationStructureFlagsKHR::PREFER_FAST_TRACE)
                .geometries(std::slice::from_ref(&geometry));

            let prim_count = (mesh
                .indices
                .as_ref()
                .ok_or(RTMeshRenderingBuildError::NonIndexedMesh)?
                .len()
                / 3)
            .try_into()
            .map_err(|_| RTMeshRenderingBuildError::TooManyIndices)?;

            let acceleration_structure_loader = ash::extensions::khr::AccelerationStructure::new(
                &renderer.instance,
                &renderer.device,
            );
            let necessary_size = unsafe {
                acceleration_structure_loader.get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::DEVICE,
                    &geometry_info,
                    std::slice::from_ref(&prim_count),
                )
            };

            let scratch_buffer = AllocatedBuffer::builder(necessary_size.build_scratch_size)
                .with_usage(
                    vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                        | vk::BufferUsageFlags::STORAGE_BUFFER
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                )
                .build(renderer)?;
            let sb_info = vk::BufferDeviceAddressInfo::builder().buffer(scratch_buffer.handle);
            let scratch_address = unsafe { renderer.device.get_buffer_device_address(&sb_info) };

            let data_buffer = AllocatedBuffer::builder(necessary_size.acceleration_structure_size)
                .with_usage(
                    vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                )
                .build(renderer)?;

            let acceleration_structure_create_info =
                vk::AccelerationStructureCreateInfoKHR::builder()
                    .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                    .size(necessary_size.acceleration_structure_size)
                    .buffer(data_buffer.handle);

            let acceleration_structure = unsafe {
                acceleration_structure_loader
                    .create_acceleration_structure(&acceleration_structure_create_info, None)
                    .map_err(RTMeshRenderingBuildError::AccelStructureCreationFailed)?
            };

            let geometry_info = geometry_info
                .dst_acceleration_structure(acceleration_structure)
                .scratch_data(vk::DeviceOrHostAddressKHR {
                    device_address: scratch_address,
                });

            let offset = vk::AccelerationStructureBuildRangeInfoKHR::builder()
                .primitive_count(prim_count)
                .primitive_offset(VertexType::position_offset());
            renderer
                .immediate_command(|cmd_buffer| unsafe {
                    acceleration_structure_loader.cmd_build_acceleration_structures(
                        *cmd_buffer,
                        std::slice::from_ref(&geometry_info),
                        std::slice::from_ref(&std::slice::from_ref(&offset)),
                    )
                })
                .map_err(RTMeshRenderingBuildError::BLASBuildingFailed)?;
        }

        Ok(ThreadSafeRef::new(Self { mesh_ref }))
    }
}
