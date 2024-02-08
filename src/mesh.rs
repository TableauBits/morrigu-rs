use ash::vk;
use bytemuck::cast_slice;
use thiserror::Error;

use crate::{
    allocated_types::{AllocatedBuffer, BufferBuildError},
    material::Vertex,
    renderer::Renderer,
    utils::ImmediateCommandError,
};

#[derive(Debug)]
pub struct Mesh<VertexType>
where
    VertexType: Vertex,
{
    pub vertices: Vec<VertexType>,
    pub indices: Option<Vec<u32>>,
    pub vertex_buffer: AllocatedBuffer,
    pub index_buffer: Option<AllocatedBuffer>,
}

impl<VertexType> Mesh<VertexType>
where
    VertexType: Vertex,
{
    pub fn destroy(&mut self, renderer: &mut Renderer) {
        if let Some(index_buffer) = self.index_buffer.as_mut() {
            index_buffer.destroy(&renderer.device, &mut renderer.allocator());
        }
        self.vertex_buffer
            .destroy(&renderer.device, &mut renderer.allocator());
    }
}

pub struct UploadData {
    pub vertex_buffer: AllocatedBuffer,
    pub index_buffer: AllocatedBuffer,
}

#[derive(Error, Debug)]
pub enum UploadError {
    #[error("Creation of staging buffer failed with error: {0}.")]
    StagingBufferCreationFailed(BufferBuildError),

    #[error(
        "Unable to find the staging buffer's allocation. This is most likely due to a use after free."
    )]
    UseAfterFree,

    #[error("Failed to map the memory of the staging buffer.")]
    MemoryMappingFailed,

    #[error("Creation of main buffer failed with error: {0}.")]
    MainBufferCreationFailed(BufferBuildError),

    #[error("Execution of copy command failed with error: {0}.")]
    CopyCommandFailed(ImmediateCommandError),
}

pub fn upload_vertex_buffer<VertexType>(
    vertices: &[VertexType],
    renderer: &mut Renderer,
) -> Result<AllocatedBuffer, UploadError>
where
    VertexType: Vertex,
{
    let vertex_data_size: u64 = std::mem::size_of_val(vertices).try_into().unwrap();
    let mut vertex_staging_buffer = AllocatedBuffer::builder(vertex_data_size)
        .with_usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .with_memory_location(gpu_allocator::MemoryLocation::CpuToGpu)
        .build(renderer)
        .map_err(UploadError::StagingBufferCreationFailed)?;

    // We cannot cast this vertex slice using bytemuck because we don't want to enforce that a vertex types doesn't have padding.
    // Padding issues are not a problem because of the way input bindings are setup (using offsets into a struct).
    // So instead, we swallow our pride, pray for forgiveness for our sins, and go to unsafe land. One more time can't hurt, right ?
    // Well I'm pretty sure it can. I've looked at this a bunch of time, and while I know for sure there's a problem in there,
    // I can't find it, so it will have to do for now.
    let vertex_staging_ptr = vertex_staging_buffer
        .allocation
        .as_ref()
        .ok_or(UploadError::UseAfterFree)?
        .mapped_ptr()
        .ok_or(UploadError::MemoryMappingFailed)?
        .cast::<VertexType>()
        .as_ptr();

    unsafe {
        std::ptr::copy_nonoverlapping(vertices.as_ptr(), vertex_staging_ptr, vertices.len());
    };

    let mut buffer_usage_flags =
        vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER;
    if cfg!(feature = "ray_tracing") {
        buffer_usage_flags |= vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        buffer_usage_flags |=
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR;
    }

    let vertex_buffer = AllocatedBuffer::builder(vertex_data_size)
        .with_usage(buffer_usage_flags)
        .with_memory_location(gpu_allocator::MemoryLocation::GpuOnly)
        .build(renderer)
        .map_err(UploadError::MainBufferCreationFailed)?;

    renderer
        .immediate_command(|cmd_buffer| {
            let copy_info = vk::BufferCopy::builder().size(vertex_data_size);

            unsafe {
                renderer.device.cmd_copy_buffer(
                    *cmd_buffer,
                    vertex_staging_buffer.handle,
                    vertex_buffer.handle,
                    std::slice::from_ref(&copy_info),
                );
            }
        })
        .map_err(UploadError::CopyCommandFailed)?;

    vertex_staging_buffer.destroy(&renderer.device, &mut renderer.allocator());

    Ok(vertex_buffer)
}

pub fn upload_index_buffer(
    indices: &[u32],
    renderer: &mut Renderer,
) -> Result<AllocatedBuffer, UploadError> {
    let index_data_size: u64 = std::mem::size_of_val(indices).try_into().unwrap();
    let mut index_staging_buffer = AllocatedBuffer::builder(index_data_size)
        .with_usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .with_memory_location(gpu_allocator::MemoryLocation::CpuToGpu)
        .build(renderer)
        .map_err(UploadError::StagingBufferCreationFailed)?;

    let raw_indices = cast_slice(indices);
    index_staging_buffer
        .allocation
        .as_mut()
        .ok_or(UploadError::UseAfterFree)?
        .mapped_slice_mut()
        .ok_or(UploadError::MemoryMappingFailed)?[..raw_indices.len()]
        .copy_from_slice(raw_indices);

    let mut buffer_usage_flags =
        vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER;
    if cfg!(feature = "ray_tracing") {
        buffer_usage_flags |= vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;
        buffer_usage_flags |=
            vk::BufferUsageFlags::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR;
    }

    let index_buffer = AllocatedBuffer::builder(index_data_size)
        .with_usage(buffer_usage_flags)
        .with_memory_location(gpu_allocator::MemoryLocation::GpuOnly)
        .build(renderer)
        .map_err(UploadError::MainBufferCreationFailed)?;

    renderer
        .immediate_command(|cmd_buffer| {
            let copy_info = vk::BufferCopy::builder().size(index_data_size);

            unsafe {
                renderer.device.cmd_copy_buffer(
                    *cmd_buffer,
                    index_staging_buffer.handle,
                    index_buffer.handle,
                    std::slice::from_ref(&copy_info),
                );
            }
        })
        .map_err(UploadError::CopyCommandFailed)?;

    index_staging_buffer.destroy(&renderer.device, &mut renderer.allocator());

    Ok(index_buffer)
}

#[derive(Error, Debug)]
pub enum MeshDataUploadError {
    #[error("Upload of mesh's vertex data failed with error: {0}.")]
    VertexBufferUploadFailed(UploadError),

    #[error("Upload of mesh's index data failed with error: {0}.")]
    IndexBufferUploadFailed(UploadError),
}

pub fn upload_mesh_data<VertexType>(
    vertices: &[VertexType],
    indices: &[u32],
    renderer: &mut Renderer,
) -> Result<UploadData, MeshDataUploadError>
where
    VertexType: Vertex,
{
    let vertex_buffer = upload_vertex_buffer(vertices, renderer)
        .map_err(MeshDataUploadError::VertexBufferUploadFailed)?;
    let index_buffer = upload_index_buffer(indices, renderer)
        .map_err(MeshDataUploadError::IndexBufferUploadFailed)?;

    Ok(UploadData {
        vertex_buffer,
        index_buffer,
    })
}
