use crate::{
    components::mesh_rendering::MeshRendering,
    error::Error,
    material::{Material, MaterialBuilder, Vertex, VertexInputDescription},
    mesh::{upload_mesh_data, Mesh, UploadResult},
    renderer::Renderer,
    shader::Shader,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
};

use ash::vk;
use bytemuck::{bytes_of, Pod, Zeroable};
use egui::Rect;
use nalgebra_glm as glm;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct EguiVertex {
    position: glm::Vec2,
    texture_coords: glm::Vec2,
    color: glm::Vec4,
}
unsafe impl Zeroable for EguiVertex {}
unsafe impl Pod for EguiVertex {}

impl Vertex for EguiVertex {
    fn vertex_input_description() -> crate::material::VertexInputDescription {
        let main_binding = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(
                std::mem::size_of::<EguiVertex>()
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .input_rate(vk::VertexInputRate::VERTEX)
            .build();

        let position = vk::VertexInputAttributeDescription::builder()
            .location(0)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(
                memoffset::offset_of!(EguiVertex, position)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        let texture_coords = vk::VertexInputAttributeDescription::builder()
            .location(1)
            .binding(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(
                memoffset::offset_of!(EguiVertex, texture_coords)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        let color = vk::VertexInputAttributeDescription::builder()
            .location(2)
            .binding(0)
            .format(vk::Format::R32G32B32A32_SFLOAT)
            .offset(
                memoffset::offset_of!(EguiVertex, color)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        VertexInputDescription {
            bindings: vec![main_binding],
            attributes: vec![position, texture_coords, color],
        }
    }
}

pub struct Painter {
    pub max_texture_size: usize,

    material: ThreadSafeRef<Material<EguiVertex>>,

    textures: std::collections::HashMap<egui::TextureId, ThreadSafeRef<Texture>>,
    frame_meshes: Vec<ThreadSafeRef<MeshRendering<EguiVertex>>>,
}

impl Painter {
    pub fn new(renderer: &mut Renderer) -> Result<Self, Error> {
        let max_texture_size = renderer
            .device_properties
            .limits
            .max_image_dimension2_d
            .try_into()
            .expect("Architecture should support u32 -> usize conversion");
        let shader = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/egui.vert"),
            include_bytes!("shaders/gen/egui.frag"),
            &renderer.device,
        )?;
        let material = MaterialBuilder::new().build(&shader, renderer)?;

        Ok(Self {
            max_texture_size,
            material,
            textures: Default::default(),
            frame_meshes: Default::default(),
        })
    }

    pub fn paint_and_update_textures(
        &mut self,
        pixels_per_point: f32,
        clipped_primitives: &[egui::ClippedPrimitive],
        textures_delta: egui::TexturesDelta,
        renderer: &mut Renderer,
    ) {
        for (id, image_delta) in textures_delta.set {
            self.set_texture(id, &image_delta, renderer);
        }

        self.paint_primitives(pixels_per_point, clipped_primitives, renderer);

        for id in textures_delta.free {
            self.free_texture(id, renderer);
        }
    }

    fn paint_primitives(
        &mut self,
        pixels_per_point: f32,
        clipped_primitives: &[egui::ClippedPrimitive],
        renderer: &mut Renderer,
    ) {
        for egui::ClippedPrimitive {
            clip_rect,
            primitive,
        } in clipped_primitives
        {
            match primitive {
                egui::epaint::Primitive::Mesh(mesh) => {
                    self.paint_mesh(pixels_per_point, clip_rect, mesh, renderer)
                }
                egui::epaint::Primitive::Callback(_) => {
                    todo!("Custom rendering callback not implemented yet")
                }
            }
        }
    }

    fn paint_mesh(
        &mut self,
        pixels_per_point: f32,
        clip_rect: &Rect,
        mesh: &egui::Mesh,
        renderer: &mut Renderer,
    ) {
        assert!(mesh.is_valid());

        let vertices: &[EguiVertex] = &mesh
            .vertices
            .iter()
            .map(|vertex| EguiVertex {
                position: glm::vec2(
                    vertex.pos.x,
                    renderer.framebuffer_height as f32 - vertex.pos.y,
                ),
                texture_coords: glm::vec2(vertex.uv.x, vertex.uv.y),
                color: glm::vec4(
                    vertex.color.r() as f32 / u8::MAX as f32,
                    vertex.color.g() as f32 / u8::MAX as f32,
                    vertex.color.b() as f32 / u8::MAX as f32,
                    vertex.color.a() as f32 / u8::MAX as f32,
                ),
            })
            .collect::<Vec<_>>();
        let UploadResult {
            vertex_buffer,
            index_buffer,
        } = upload_mesh_data(vertices, &mesh.indices, renderer)
            .expect("Failed to upload egui mesh data");
        let mesh_ref = ThreadSafeRef::new(Mesh {
            vertices: vertices.to_vec(),
            indices: Some(mesh.indices.clone()),
            vertex_buffer,
            index_buffer: Some(index_buffer),
        });

        let width = renderer.framebuffer_width as f32 / pixels_per_point;
        let height = renderer.framebuffer_height as f32 / pixels_per_point;

        let texture = self.textures.get(&mesh.texture_id);
        if texture.is_none() {
            return;
        }
        let texture = texture.unwrap();
        let push_constants = glm::vec2(width, height);

        let mesh_rendering_ref = MeshRendering::new(&mesh_ref, &self.material, renderer)
            .expect("Failed to create mesh rendering for egui mesh");
        let mut mesh_rendering = mesh_rendering_ref.lock();
        mesh_rendering
            .bind_texture(1, texture, renderer)
            .expect("Texture binding for Egui should succeed")
            .lock()
            .destroy(renderer);

        let device = &renderer.device;
        let cmd_buffer = &renderer.primary_command_buffer;
        let material = self.material.lock();
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
        let y: f32 = u16::try_from(renderer.framebuffer_height)
            .expect("Invalid width")
            .into();

        let viewport = vk::Viewport::builder()
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

        let min_x = (pixels_per_point * clip_rect.min.x)
            .clamp(0.0, width)
            .round();
        let min_y = (pixels_per_point * clip_rect.min.y)
            .clamp(0.0, height)
            .round();
        let max_x = (pixels_per_point * clip_rect.max.x)
            .clamp(pixels_per_point * clip_rect.min.x, width)
            .round();
        let max_y = (pixels_per_point * clip_rect.max.y)
            .clamp(pixels_per_point * clip_rect.min.y, height)
            .round();
        let scissor = vk::Rect2D::builder()
            .offset(vk::Offset2D {
                x: min_x as i32,
                y: min_y as i32,
            })
            .extent(vk::Extent2D {
                width: (max_x - min_x) as u32,
                height: (max_y - min_y) as u32,
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
            device.cmd_push_constants(
                *cmd_buffer,
                material.layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                bytes_of(&push_constants),
            );

            device.cmd_bind_descriptor_sets(
                *cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                material.layout,
                3,
                std::slice::from_ref(&mesh_rendering.descriptor_set),
                &[],
            );

            let mesh = mesh_ref.lock();
            device.cmd_bind_vertex_buffers(
                *cmd_buffer,
                0,
                std::slice::from_ref(&mesh.vertex_buffer.handle),
                &[0],
            );
            device.cmd_bind_index_buffer(
                *cmd_buffer,
                mesh.index_buffer.as_ref().unwrap().handle,
                0,
                vk::IndexType::UINT32,
            );
            device.cmd_draw_indexed(
                *cmd_buffer,
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
        };

        drop(mesh_rendering);
        self.frame_meshes.push(mesh_rendering_ref);
    }

    pub fn cleanup_previous_frame(&mut self, renderer: &mut Renderer) {
        for mesh_rendering_ref in &self.frame_meshes {
            let mut mesh_rendering = mesh_rendering_ref.lock();
            mesh_rendering.mesh_ref.lock().destroy(renderer);
            mesh_rendering.destroy(renderer);
        }

        self.frame_meshes.clear();
    }

    fn set_texture(
        &mut self,
        tex_id: egui::TextureId,
        delta: &egui::epaint::ImageDelta,
        renderer: &mut Renderer,
    ) {
        let pixels: Vec<u8> = match &delta.image {
            egui::ImageData::Color(image) => image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect(),
            egui::ImageData::Font(image) => image
                .srgba_pixels(1.0)
                .flat_map(|pixel| pixel.to_array())
                .collect(),
        };
        let texture = Texture::builder()
            .with_format(TextureFormat::RGBA8_SRGB)
            .build_from_data(
                &pixels,
                delta
                    .image
                    .width()
                    .try_into()
                    .expect("Architecture should support usize -> u32 conversion"),
                delta
                    .image
                    .height()
                    .try_into()
                    .expect("Architecture should support usize -> u32 conversion"),
                renderer,
            )
            .expect("Failed to create egui texture");

        match delta.pos {
            Some(pos) => {
                let original_texture = self.textures.get(&tex_id);
                if original_texture.is_none() {
                    return;
                }
                let original_texture = original_texture.unwrap().lock();

                let mut texture = texture.lock();
                let subresource = vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                };
                let copy_region = vk::ImageCopy::builder()
                    .src_subresource(subresource)
                    .dst_subresource(subresource)
                    .dst_offset(vk::Offset3D {
                        x: pos[0].try_into().expect("Egui error: Texture too large!!!"),
                        y: pos[1].try_into().expect("Egui error: Texture too large!!!"),
                        z: 0,
                    })
                    .extent(vk::Extent3D {
                        width: texture.dimensions[0],
                        height: texture.dimensions[1],
                        depth: 1,
                    });

                let range = vk::ImageSubresourceRange::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1);
                let transfer_src_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::NONE)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                    .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .image(texture.image.handle)
                    .subresource_range(*range);
                let transfer_dst_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::NONE)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .image(original_texture.image.handle)
                    .subresource_range(*range);

                let shader_read_src_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image(texture.image.handle)
                    .subresource_range(*range);
                let shader_read_dst_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image(original_texture.image.handle)
                    .subresource_range(*range);

                renderer
                    .immediate_command(|cmd_buffer| {
                        unsafe {
                            renderer.device.cmd_pipeline_barrier(
                                *cmd_buffer,
                                vk::PipelineStageFlags::TOP_OF_PIPE,
                                vk::PipelineStageFlags::TRANSFER,
                                vk::DependencyFlags::empty(),
                                &[],
                                &[],
                                &[*transfer_src_barrier, *transfer_dst_barrier],
                            );
                            renderer.device.cmd_copy_image(
                                *cmd_buffer,
                                texture.image.handle,
                                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                                original_texture.image.handle,
                                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                                std::slice::from_ref(&copy_region),
                            );

                            renderer.device.cmd_pipeline_barrier(
                                *cmd_buffer,
                                vk::PipelineStageFlags::TRANSFER,
                                vk::PipelineStageFlags::FRAGMENT_SHADER,
                                vk::DependencyFlags::empty(),
                                &[],
                                &[],
                                &[*shader_read_src_barrier, *shader_read_dst_barrier],
                            );
                        };
                    })
                    .expect("Failed to update Egui image");

                texture.destroy(renderer);
            }
            None => {
                self.textures.insert(tex_id, texture);
            }
        }
    }

    fn free_texture(&mut self, tex_id: egui::TextureId, renderer: &mut Renderer) {
        if let Some(texture) = self.textures.remove(&tex_id) {
            texture.lock().destroy(renderer);
        }
    }

    pub(crate) fn destroy(&mut self, renderer: &mut Renderer) {
        self.cleanup_previous_frame(renderer);

        for (_, texture) in self.textures.drain() {
            texture.lock().destroy(renderer);
        }

        let mut material = self.material.lock();
        material.shader_ref.lock().destroy(&renderer.device);
        material.destroy(renderer);
    }
}
