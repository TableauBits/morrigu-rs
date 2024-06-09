use std::time::Instant;

use crate::{
    components::{
        camera::Camera, mesh_rendering::MeshRendering, resource_wrapper::ResourceWrapper,
        transform::Transform,
    },
    material::{Material, Vertex},
    math_types::{Mat4, Vec4},
    renderer::Renderer,
    utils::ThreadSafeRef,
};

use ash::vk;
use bevy_ecs::{prelude::Query, system::Res};
use bytemuck::{bytes_of, Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct CameraData {
    pub(crate) view_projection: Mat4,
    pub(crate) world_position: Vec4,
}
unsafe impl Zeroable for CameraData {}
unsafe impl Pod for CameraData {}

#[profiling::function]
pub fn render_meshes<VertexType>(
    query: Query<(&Transform, &ThreadSafeRef<MeshRendering<VertexType>>)>,
    timer: Res<ResourceWrapper<Instant>>,
    camera: Res<Camera>,
    renderer_ref: Res<ThreadSafeRef<Renderer>>,
) where
    VertexType: Vertex,
{
    let timer = timer.data;
    let mut renderer = renderer_ref.lock();

    let current_time = timer.elapsed().as_secs_f32();
    let time_data = Vec4::new(
        current_time / 20.0,
        current_time,
        current_time * 2.0,
        current_time * 3.0,
    );

    let time_buffer = renderer.descriptors[0].buffer.as_mut().unwrap();

    let raw_time_data = bytes_of(&time_data);
    time_buffer
        .allocation
        .as_mut()
        .expect("Free after use")
        .mapped_slice_mut()
        .expect("Memory should be mappable")[..raw_time_data.len()]
        .copy_from_slice(raw_time_data);

    let mut last_material: Option<ThreadSafeRef<Material<VertexType>>> = None;
    let mut last_material_pipeline: Option<vk::Pipeline> = None;
    let device = renderer.device.clone();
    let cmd_buffer = renderer.primary_command_buffer;
    for (transform, mesh_rendering_ref) in query.iter() {
        let mut mesh_rendering = mesh_rendering_ref.lock();

        if !mesh_rendering.visible {
            continue;
        };

        if mesh_rendering
            .update_uniform_pod(0, transform.matrix())
            .is_err()
        {
            log::warn!("Failed to upload model data to slot 0");
        }

        let material = mesh_rendering.material_ref.lock();
        let mesh = mesh_rendering.mesh_ref.lock();

        if last_material.is_none() {
            // first draw, need to bind the descriptor set (common for all materials)
            unsafe {
                device.cmd_bind_descriptor_sets(
                    cmd_buffer,
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
            material
                .descriptor_resources
                .prepare_image_layouts_for_render(&mut renderer)
                .expect("Failed to prepare images for draw");

            // This one small trick allows us to keep vertex data sane
            // (Actual engineers hate him)
            // This is also why we had to bump to requesting 1.1.0 lmao
            // https://www.saschawillems.de/blog/2019/03/29/flipping-the-vulkan-viewport/
            let y: f32 = u16::try_from(renderer.framebuffer_height)
                .expect("Invalid width")
                .into();

            let viewport = vk::Viewport::default()
                .x(0.0)
                .y(y)
                .width(
                    u16::try_from(renderer.framebuffer_width)
                        .expect("Invalid width")
                        .into(),
                )
                .height(-y)
                .min_depth(0.0)
                .max_depth(1.0);
            let scissor = vk::Rect2D::default()
                .offset(vk::Offset2D::default())
                .extent(vk::Extent2D {
                    width: renderer.framebuffer_width,
                    height: renderer.framebuffer_height,
                });
            unsafe {
                device.cmd_bind_pipeline(
                    cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    material.pipeline,
                );
                device.cmd_set_viewport(cmd_buffer, 0, std::slice::from_ref(&viewport));
                device.cmd_set_scissor(cmd_buffer, 0, std::slice::from_ref(&scissor));
                device.cmd_bind_descriptor_sets(
                    cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    material.layout,
                    2,
                    std::slice::from_ref(&material.descriptor_set),
                    &[],
                );
            };

            last_material_pipeline = Some(material.pipeline);
            if let Some(last_material) = last_material {
                last_material
                    .lock()
                    .descriptor_resources
                    .restore_image_layouts(&mut renderer)
                    .expect("Failed to restore image layouts");
            }
            last_material = Some(mesh_rendering.material_ref.clone());
        }

        let camera_data = CameraData {
            view_projection: *camera.view_projection(),
            world_position: (*camera.position(), 1.0).into(),
        };

        unsafe {
            device.cmd_push_constants(
                cmd_buffer,
                material.layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytes_of(&camera_data),
            );

            device.cmd_bind_descriptor_sets(
                cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                material.layout,
                3,
                std::slice::from_ref(&mesh_rendering.descriptor_set),
                &[],
            );

            device.cmd_bind_vertex_buffers(
                cmd_buffer,
                0,
                std::slice::from_ref(&mesh.vertex_buffer.handle),
                &[0],
            );
            match mesh.index_buffer.as_ref() {
                Some(index_buffer) => {
                    device.cmd_bind_index_buffer(
                        cmd_buffer,
                        index_buffer.handle,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_draw_indexed(
                        cmd_buffer,
                        mesh.indices
                            .as_ref()
                            .unwrap()
                            .len()
                            .try_into()
                            .expect("Unsupported architecture"),
                        1,
                        0,
                        0,
                        0,
                    );
                }
                None => {
                    device.cmd_draw(
                        cmd_buffer,
                        mesh.vertices
                            .len()
                            .try_into()
                            .expect("Unsupported architecture"),
                        1,
                        0,
                        0,
                    );
                }
            }
        }
    }
}
