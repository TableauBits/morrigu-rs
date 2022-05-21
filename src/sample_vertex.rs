use ash::vk;
use nalgebra_glm as glm;

use crate::{
    allocated_types::AllocatedBuffer,
    error::Error,
    material::{Vertex, VertexInputDescription},
    mesh::Mesh,
    renderer::Renderer,
    utils::ThreadSafeRef,
};

#[repr(C)]
pub struct TexturedVertex {
    position: glm::Vec3,
    normal: glm::Vec3,
    texture_coords: glm::Vec2,
}

impl Vertex for TexturedVertex {
    fn vertex_input_description() -> VertexInputDescription {
        let main_binding = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(
                std::mem::size_of::<TexturedVertex>()
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .input_rate(vk::VertexInputRate::VERTEX)
            .build();

        let position = vk::VertexInputAttributeDescription::builder()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(
                memoffset::offset_of!(TexturedVertex, position)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        let normal = vk::VertexInputAttributeDescription::builder()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(
                memoffset::offset_of!(TexturedVertex, normal)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        let texture_coords = vk::VertexInputAttributeDescription::builder()
            .location(2)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(
                memoffset::offset_of!(TexturedVertex, texture_coords)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        VertexInputDescription {
            bindings: vec![main_binding],
            attributes: vec![position, normal, texture_coords],
        }
    }
}

impl TexturedVertex {
    pub fn load_model_from_path(
        path: &std::path::Path,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Mesh<Self>>, Error> {
        let (load_result, _) = tobj::load_obj(
            path,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
        )?;

        let mesh = &load_result[0].mesh;

        let positions = mesh
            .positions
            .chunks_exact(3)
            .map(|slice| glm::Vec3::new(slice[0], slice[1], slice[2]))
            .collect::<Vec<glm::Vec3>>();
        let normals = mesh
            .normals
            .chunks_exact(3)
            .map(|slice| glm::Vec3::new(slice[0], slice[1], slice[2]))
            .collect::<Vec<glm::Vec3>>();
        let texture_coordinates = mesh
            .texcoords
            .chunks_exact(2)
            .map(|slice| glm::Vec2::new(slice[0], slice[1]))
            .collect::<Vec<glm::Vec2>>();

        let mut vertices = Vec::with_capacity(positions.len());
        for index in 0..positions.len() {
            vertices.push(TexturedVertex {
                position: positions[index],
                normal: normals[index],
                texture_coords: texture_coordinates[index],
            });
        }

        let indices = mesh.indices.clone();

        let vertex_data_size: u64 = (vertices.len() * std::mem::size_of::<Self>()).try_into()?;
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
            .cast::<Self>()
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

        Ok(ThreadSafeRef::new(Mesh::<Self> {
            vertices,
            indices,
            vertex_buffer,
            index_buffer,
        }))
    }
}
