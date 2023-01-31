use crate::{
    components::mesh_rendering::MeshRendering,
    descriptor_resources::DescriptorResources,
    material::{Material, MaterialBuilder, Vertex, VertexInputDescription, MaterialBuildError},
    mesh::{upload_mesh_data, Mesh, UploadResult},
    renderer::Renderer,
    shader::{Shader, ShaderBuildError},
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
    vector_type::{Vec2, Vec4},
};

use ash::vk;
use bytemuck::{bytes_of, Pod, Zeroable};
use egui::Rect;
use thiserror::Error;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct EguiVertex {
    position: Vec2,
    texture_coords: Vec2,
    color: Vec4,
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

struct TextureInfo {
    handle: ThreadSafeRef<Texture>,
    is_user: bool,
}

pub struct Painter {
    pub max_texture_size: usize,

    material: ThreadSafeRef<Material<EguiVertex>>,

    textures: std::collections::HashMap<egui::TextureId, TextureInfo>,
    frame_meshes: Vec<ThreadSafeRef<MeshRendering<EguiVertex>>>,
    user_texture_id: u64,
}

#[derive(Error, Debug)]
pub enum PainterCreationError {
    #[error("Conversion of size max image dimensions from u32 to usize failed (check that {0} <= usize::MAX).")]
    SizeConversionFailed(u32),

    #[error("Creation of egui shader failed with error: {0}.")]
    ShaderCreationFailed(#[from] ShaderBuildError),

    #[error("Creation of egui material failed with error: {0}.")]
    MaterialCreationFailed(#[from] MaterialBuildError),
}

impl Painter {
    pub fn new(renderer: &mut Renderer) -> Result<Self, PainterCreationError> {
        let max_texture_size = renderer
            .device_properties
            .limits
            .max_image_dimension2_d
            .try_into()
            .map_err(|_| {
                PainterCreationError::SizeConversionFailed(
                    renderer.device_properties.limits.max_image_dimension2_d,
                )
            })?;
        let shader = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/egui.vert"),
            include_bytes!("shaders/gen/egui.frag"),
            &renderer.device,
        )?;
        let material =
            MaterialBuilder::new().build(&shader, DescriptorResources::empty(), renderer)?;

        Ok(Self {
            max_texture_size,
            material,
            textures: Default::default(),
            frame_meshes: Default::default(),
            user_texture_id: 0,
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
        if mesh.is_empty() {
            return;
        }

        let width = renderer.framebuffer_width as f32;
        let height = renderer.framebuffer_height as f32;
        let width_in_points = width / pixels_per_point;
        let height_in_points = height / pixels_per_point;

        let vertices: &[EguiVertex] = &mesh
            .vertices
            .iter()
            .map(|vertex| EguiVertex {
                position: Vec2::new(vertex.pos.x, height_in_points - vertex.pos.y),
                texture_coords: Vec2::new(vertex.uv.x, vertex.uv.y),
                color: Vec4::new(
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

        let texture = self.textures.get(&mesh.texture_id);
        if texture.is_none() {
            return;
        }
        let texture = texture.unwrap();
        let push_constants = Vec2::new(width_in_points, height_in_points);

        let mesh_rendering_ref = MeshRendering::new(
            &mesh_ref,
            &self.material,
            DescriptorResources {
                sampled_images: [(1, texture.handle.clone())].into(),
                ..Default::default()
            },
            renderer,
        )
        .expect("Failed to create mesh rendering for egui mesh");
        let mut mesh_rendering = mesh_rendering_ref.lock();
        mesh_rendering
            .bind_texture(1, texture.handle.clone(), renderer)
            .expect("Texture binding for Egui should succeed");

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

        let viewport = vk::Viewport::builder()
            .x(0.0)
            .y(height)
            .width(width)
            .height(-height)
            .min_depth(0.0)
            .max_depth(1.0);

        let min_x = pixels_per_point * clip_rect.min.x;
        let min_y = pixels_per_point * clip_rect.min.y;
        let max_x = pixels_per_point * clip_rect.max.x;
        let max_y = pixels_per_point * clip_rect.max.y;

        let min_x = min_x.clamp(0.0, width);
        let min_y = min_y.clamp(0.0, height);
        let max_x = max_x.clamp(min_x, width);
        let max_y = max_y.clamp(min_y, height);

        let min_x = min_x.round() as u32;
        let min_y = min_y.round() as u32;
        let max_x = max_x.round() as u32;
        let max_y = max_y.round() as u32;

        let scissor = vk::Rect2D::builder()
            .offset(vk::Offset2D {
                x: min_x as i32,
                y: min_y as i32,
            })
            .extent(vk::Extent2D {
                width: max_x - min_x,
                height: max_y - min_y,
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
                .srgba_pixels(None)
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
                let original_texture = original_texture.unwrap().handle.lock();

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

                let texture_image = texture.image_ref.lock();
                let original_texture_image = original_texture.image_ref.lock();

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
                    .image(texture_image.handle)
                    .subresource_range(*range);
                let transfer_dst_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::NONE)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .image(original_texture_image.handle)
                    .subresource_range(*range);

                let shader_read_src_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::TRANSFER_READ)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image(texture_image.handle)
                    .subresource_range(*range);
                let shader_read_dst_barrier = vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image(original_texture_image.handle)
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
                                texture_image.handle,
                                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                                original_texture_image.handle,
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

                drop(texture_image);
                texture.destroy(renderer);
            }
            None => {
                self.textures.insert(
                    tex_id,
                    TextureInfo {
                        handle: texture,
                        is_user: false,
                    },
                );
            }
        }
    }

    pub(crate) fn free_texture(&mut self, tex_id: egui::TextureId, renderer: &mut Renderer) {
        if let Some(TextureInfo { handle, .. }) = self.textures.remove(&tex_id) {
            handle.lock().destroy(renderer);
        }
    }

    pub fn register_user_texture(&mut self, texture: ThreadSafeRef<Texture>) -> egui::TextureId {
        let id = egui::TextureId::User(self.user_texture_id);
        self.user_texture_id += 1;

        self.textures.insert(
            id,
            TextureInfo {
                handle: texture,
                is_user: true,
            },
        );

        id
    }

    pub fn retreive_user_texture(
        &mut self,
        tex_id: egui::TextureId,
    ) -> Option<ThreadSafeRef<Texture>> {
        self.textures.remove(&tex_id).map(|info| info.handle)
    }

    pub fn replace_user_texture(
        &mut self,
        tex_id: egui::TextureId,
        new_texture: ThreadSafeRef<Texture>,
    ) -> Option<ThreadSafeRef<Texture>> {
        self.textures
            .insert(
                tex_id,
                TextureInfo {
                    handle: new_texture,
                    is_user: true,
                },
            )
            .map(|info| info.handle)
    }

    pub(crate) fn destroy(&mut self, renderer: &mut Renderer) {
        self.cleanup_previous_frame(renderer);

        for (_, TextureInfo { handle, is_user }) in self.textures.drain() {
            if !is_user {
                handle.lock().destroy(renderer);
            }
        }

        let mut material = self.material.lock();
        material.shader_ref.lock().destroy(&renderer.device);
        material.destroy(renderer);
    }
}
