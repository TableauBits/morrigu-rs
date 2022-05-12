use std::time::Instant;

use crate::{
    components::{camera::Camera, mesh_rendering::MeshRendering, transform::Transform},
    material::Vertex,
    renderer::{Renderer, TimeData},
    utils::ThreadSafeRef,
};

use ash::vk;
use bevy_ecs::{prelude::Query, system::Res};
use nalgebra_glm as glm;

#[repr(C)]
#[derive(Debug)]
struct CameraData {
    pub(crate) view_projection: glm::Mat4,
    pub(crate) world_position: glm::Vec4,
}

pub fn render_meshes<VertexType>(
    query: Query<(&Transform, &ThreadSafeRef<MeshRendering<VertexType>>)>,
    timer: Res<Instant>,
    camera: Res<Camera>,
    renderer_ref: Res<ThreadSafeRef<Renderer>>,
) where
    VertexType: Vertex,
{
    let renderer = renderer_ref.lock();

    let current_time = timer.elapsed().as_secs_f32();
    let time_data = TimeData {
        time: glm::Vec4::new(
            current_time / 20.0,
            current_time,
            current_time * 2.0,
            current_time * 3.0,
        ),
    };
    let time_buffer = renderer.descriptors[0].buffer.as_ref().unwrap();

    let dst_ptr = time_buffer
        .allocation
        .as_ref()
        .expect("Free after use")
        .mapped_ptr()
        .expect("Failed to map memory")
        .cast::<TimeData>()
        .as_ptr();
    unsafe { std::ptr::copy_nonoverlapping(&time_data, dst_ptr, 1) };

    let mut last_material_pipeline = None;
    let device = &renderer.device;
    let cmd_buffer = &renderer.primary_command_buffer;
    for (transform, mesh_rendering_ref) in query.iter() {
        let mesh_rendering = mesh_rendering_ref.lock();
        let material = mesh_rendering.material_ref.lock();
        let mesh = mesh_rendering.mesh_ref.lock();

        let upload_result = mesh_rendering.upload_uniform(0, *transform.matrix());
        if upload_result.is_err() {
            log::warn!("Failed to upload model data to slot 0");
        }

        if let None = last_material_pipeline {
            // first draw, need to bind the descriptor set (common for all materials)
            unsafe {
                device.cmd_bind_descriptor_sets(
                    *cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    material.layout,
                    0,
                    &[
                        renderer.descriptors[0].handle,
                        renderer.descriptors[1].handle,
                    ],
                    &[],
                )
            };
        }
        if last_material_pipeline != Some(material.pipeline) {
            let viewport = vk::Viewport::builder()
                .x(0.0)
                .y(0.0)
                .width(
                    u16::try_from(renderer.framebuffer_width)
                        .expect("Invalid width")
                        .into(),
                )
                .height(
                    u16::try_from(renderer.framebuffer_height)
                        .expect("Invalid width")
                        .into(),
                )
                .min_depth(0.0)
                .max_depth(1.0);
            let scissor = vk::Rect2D::builder()
                .offset(vk::Offset2D::default())
                .extent(vk::Extent2D {
                    width: renderer.framebuffer_width,
                    height: renderer.framebuffer_height,
                });
            unsafe {
                device.cmd_bind_pipeline(
                    *cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    material.pipeline,
                );
                device.cmd_set_viewport(*cmd_buffer, 0, std::slice::from_ref(&viewport));
                device.cmd_set_scissor(*cmd_buffer, 0, std::slice::from_ref(&scissor));
                device.cmd_bind_descriptor_sets(
                    *cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    material.layout,
                    2,
                    std::slice::from_ref(&material.descriptor_set),
                    &[],
                );
            };

            last_material_pipeline = Some(material.pipeline.clone());
        }

        let mut camera_data = CameraData {
            view_projection: *camera.view_projection(),
            world_position: glm::vec3_to_vec4(camera.position()),
        };
        let camera_data_ptr = std::ptr::NonNull::new(&mut camera_data)
            .expect("Failed to create camera data")
            .cast::<u8>()
            .as_ptr();

        unsafe {
            let camera_data_raw =
                std::slice::from_raw_parts(camera_data_ptr, std::mem::size_of::<CameraData>());
            device.cmd_push_constants(
                *cmd_buffer,
                material.layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                camera_data_raw,
            );

            device.cmd_bind_descriptor_sets(
                *cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                material.layout,
                3,
                std::slice::from_ref(&mesh_rendering.descriptor_set),
                &[],
            );

            device.cmd_bind_vertex_buffers(
                *cmd_buffer,
                0,
                std::slice::from_ref(&mesh.vertex_buffer.handle),
                &[0],
            );
            device.cmd_bind_index_buffer(
                *cmd_buffer,
                mesh.index_buffer.handle,
                0,
                vk::IndexType::UINT32,
            );
            device.cmd_draw_indexed(
                *cmd_buffer,
                mesh.indices
                    .len()
                    .try_into()
                    .expect("Unsupported architecture"),
                1,
                0,
                0,
                0,
            );
        }
    }
}
