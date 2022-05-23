use ash::vk;

use crate::{allocated_types::AllocatedBuffer, error::Error, material::Vertex, renderer::Renderer};

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

pub struct UploadResult {
    pub vertex_buffer: AllocatedBuffer,
    pub index_buffer: AllocatedBuffer,
}

pub fn upload_vertex_buffer<VertexType>(
    vertices: &[VertexType],
    renderer: &mut Renderer,
) -> Result<AllocatedBuffer, Error>
where
    VertexType: Vertex,
{
    let vertex_data_size: u64 = (vertices.len() * std::mem::size_of::<VertexType>()).try_into()?;
    let mut vertex_staging_buffer = AllocatedBuffer::builder(vertex_data_size)
        .with_usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .with_memory_location(gpu_allocator::MemoryLocation::CpuToGpu)
        .build(&renderer.device, &mut renderer.allocator())?;
    let vertex_staging_ptr = vertex_staging_buffer
        .allocation
        .as_ref()
        .ok_or("use after free")?
        .mapped_ptr()
        .ok_or_else(|| {
            gpu_allocator::AllocationError::FailedToMap("Failed to map memory".to_owned())
        })?
        .cast::<VertexType>()
        .as_ptr();

    unsafe {
        std::ptr::copy_nonoverlapping(vertices.as_ptr(), vertex_staging_ptr, vertices.len());
    };

    let vertex_buffer = AllocatedBuffer::builder(vertex_data_size)
        .with_usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER)
        .with_memory_location(gpu_allocator::MemoryLocation::GpuOnly)
        .build(&renderer.device, &mut renderer.allocator())?;

    renderer.immediate_command(|cmd_buffer| {
        let copy_info = vk::BufferCopy::builder().size(vertex_data_size);

        unsafe {
            renderer.device.cmd_copy_buffer(
                *cmd_buffer,
                vertex_staging_buffer.handle,
                vertex_buffer.handle,
                std::slice::from_ref(&copy_info),
            );
        }
    })?;

    vertex_staging_buffer.destroy(&renderer.device, &mut renderer.allocator());

    Ok(vertex_buffer)
}

pub fn upload_index_buffer(
    indices: &[u32],
    renderer: &mut Renderer,
) -> Result<AllocatedBuffer, Error> {
    let index_data_size: u64 = (indices.len() * std::mem::size_of::<u32>()).try_into()?;
    let mut index_staging_buffer = AllocatedBuffer::builder(index_data_size)
        .with_usage(vk::BufferUsageFlags::TRANSFER_SRC)
        .with_memory_location(gpu_allocator::MemoryLocation::CpuToGpu)
        .build(&renderer.device, &mut renderer.allocator())?;

    let index_staging_ptr = index_staging_buffer
        .allocation
        .as_ref()
        .ok_or("use after free")?
        .mapped_ptr()
        .ok_or_else(|| {
            gpu_allocator::AllocationError::FailedToMap("Failed to map memory".to_owned())
        })?
        .cast::<u32>()
        .as_ptr();

    unsafe {
        std::ptr::copy_nonoverlapping(indices.as_ptr(), index_staging_ptr, indices.len());
    };

    let index_buffer = AllocatedBuffer::builder(index_data_size)
        .with_usage(vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER)
        .with_memory_location(gpu_allocator::MemoryLocation::GpuOnly)
        .build(&renderer.device, &mut renderer.allocator())?;

    renderer.immediate_command(|cmd_buffer| {
        let copy_info = vk::BufferCopy::builder().size(index_data_size);

        unsafe {
            renderer.device.cmd_copy_buffer(
                *cmd_buffer,
                index_staging_buffer.handle,
                index_buffer.handle,
                std::slice::from_ref(&copy_info),
            );
        }
    })?;

    index_staging_buffer.destroy(&renderer.device, &mut renderer.allocator());

    Ok(index_buffer)
}

pub fn upload_mesh_data<VertexType>(
    vertices: &[VertexType],
    indices: &[u32],
    renderer: &mut Renderer,
) -> Result<UploadResult, Error>
where
    VertexType: Vertex,
{
    let vertex_buffer = upload_vertex_buffer(vertices, renderer)?;
    let index_buffer = upload_index_buffer(indices, renderer)?;

    Ok(UploadResult {
        vertex_buffer,
        index_buffer,
    })
}
