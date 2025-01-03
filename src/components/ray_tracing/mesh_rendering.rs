use std::fmt;

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

#[derive(Component)]
pub struct MeshRendering<VertexType: Vertex> {
    pub mesh_ref: ThreadSafeRef<Mesh<VertexType>>,

    data_buffer: AllocatedBuffer,
    tlas_instance: vk::AccelerationStructureInstanceKHR,
    blas: vk::AccelerationStructureKHR,
}

impl<VertexType: Vertex> fmt::Debug for MeshRendering<VertexType> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MeshRendering {{mesh_ref: {:?}, blas: {:?}}}",
            self.mesh_ref, self.blas
        )
    }
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

impl<VertexType: Vertex> MeshRendering<VertexType> {
    pub fn blas(&self) -> &vk::AccelerationStructureKHR {
        &self.blas
    }

    pub fn tlas_instance(&self) -> &vk::AccelerationStructureInstanceKHR {
        &self.tlas_instance
    }

    pub fn new(
        mesh_ref: ThreadSafeRef<Mesh<VertexType>>,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Self>, RTMeshRenderingBuildError> {
        let blas;
        let tlas_instance;
        let data_buffer;

        {
            let mesh = mesh_ref.lock();

            let buffer_info =
                vk::BufferDeviceAddressInfo::default().buffer(mesh.vertex_buffer.handle);
            let vertex_address = unsafe { renderer.device.get_buffer_device_address(&buffer_info) };

            let buffer_info = buffer_info.buffer(
                mesh.index_buffer
                    .as_ref()
                    .ok_or(RTMeshRenderingBuildError::NonIndexedMesh)?
                    .handle,
            );
            let index_address = unsafe { renderer.device.get_buffer_device_address(&buffer_info) };

            let triangle_data = vk::AccelerationStructureGeometryTrianglesDataKHR::default()
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

            let geometry = vk::AccelerationStructureGeometryKHR::default()
                .geometry_type(vk::GeometryTypeKHR::TRIANGLES)
                .flags(vk::GeometryFlagsKHR::OPAQUE)
                .geometry(vk::AccelerationStructureGeometryDataKHR {
                    triangles: triangle_data,
                });
            let geometry_info = vk::AccelerationStructureBuildGeometryInfoKHR::default()
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

            let acceleration_structure_loader =
                ash::khr::acceleration_structure::Device::new(&renderer.instance, &renderer.device);
            let mut necessary_size = Default::default();
            unsafe {
                acceleration_structure_loader.get_acceleration_structure_build_sizes(
                    vk::AccelerationStructureBuildTypeKHR::DEVICE,
                    &geometry_info,
                    std::slice::from_ref(&prim_count),
                    &mut necessary_size,
                )
            };

            let mut scratch_buffer = AllocatedBuffer::builder(necessary_size.build_scratch_size)
                .with_name("BLAS scratch")
                .with_usage(
                    vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS
                        | vk::BufferUsageFlags::STORAGE_BUFFER
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                )
                .build(renderer)?;
            let sb_info = vk::BufferDeviceAddressInfo::default().buffer(scratch_buffer.handle);
            let scratch_address = unsafe { renderer.device.get_buffer_device_address(&sb_info) };

            data_buffer = AllocatedBuffer::builder(necessary_size.acceleration_structure_size)
                .with_name("BLAS data")
                .with_usage(
                    vk::BufferUsageFlags::ACCELERATION_STRUCTURE_STORAGE_KHR
                        | vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS,
                )
                .build(renderer)?;

            let acceleration_structure_create_info =
                vk::AccelerationStructureCreateInfoKHR::default()
                    .ty(vk::AccelerationStructureTypeKHR::BOTTOM_LEVEL)
                    .size(necessary_size.acceleration_structure_size)
                    .buffer(data_buffer.handle);

            blas = unsafe {
                acceleration_structure_loader
                    .create_acceleration_structure(&acceleration_structure_create_info, None)
                    .map_err(RTMeshRenderingBuildError::AccelStructureCreationFailed)?
            };

            let geometry_info = geometry_info.dst_acceleration_structure(blas).scratch_data(
                vk::DeviceOrHostAddressKHR {
                    device_address: scratch_address,
                },
            );

            let offset = vk::AccelerationStructureBuildRangeInfoKHR::default()
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

            let blas_info = vk::AccelerationStructureDeviceAddressInfoKHR::default()
                .acceleration_structure(blas);
            let blas_address = unsafe {
                acceleration_structure_loader.get_acceleration_structure_device_address(&blas_info)
            };

            tlas_instance = vk::AccelerationStructureInstanceKHR {
                transform: vk::TransformMatrixKHR {
                    matrix: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                },
                instance_custom_index_and_mask: vk::Packed24_8::new(0, 0xFF),
                instance_shader_binding_table_record_offset_and_flags: vk::Packed24_8::new(0, 1),
                acceleration_structure_reference: vk::AccelerationStructureReferenceKHR {
                    device_handle: blas_address,
                },
            };

            scratch_buffer.destroy(&renderer.device, &mut renderer.allocator());
        }

        Ok(ThreadSafeRef::new(Self {
            data_buffer,
            mesh_ref,
            blas,
            tlas_instance,
        }))
    }

    pub fn destroy(&mut self, renderer: &mut Renderer) {
        let acceleration_structure_loader =
            ash::khr::acceleration_structure::Device::new(&renderer.instance, &renderer.device);
        unsafe {
            acceleration_structure_loader.destroy_acceleration_structure(self.blas, None);
        }

        self.data_buffer
            .destroy(&renderer.device, &mut renderer.allocator())
    }
}
