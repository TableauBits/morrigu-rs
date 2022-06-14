use crate::{
    error::Error,
    material::{Material, MaterialBuilder, Vertex, VertexInputDescription},
    mesh::{upload_index_buffer, upload_vertex_buffer},
    renderer::Renderer,
    shader::Shader,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
};

use ash::vk;
use bytemuck::{cast_slice, Pod, Zeroable};
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
            .format(vk::Format::R32G32_SFLOAT)
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

    next_native_texture_id: u64,
    textures: std::collections::HashMap<egui::TextureId, ThreadSafeRef<Texture>>,
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
            next_native_texture_id: 0,
            textures: Default::default(),
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

        let vertices: &[EguiVertex] = cast_slice(&mesh.vertices);
        let vertex_buffer =
            upload_vertex_buffer(vertices, renderer).expect("Failed to create vertex buffer");
        let index_buffer =
            upload_index_buffer(&mesh.indices, renderer).expect("Failed to create index buffer");

        let width = renderer.framebuffer_width as f32 / pixels_per_point;
        let height = renderer.framebuffer_height as f32 / pixels_per_point;
		  
		let texture = self.textures.get(&mesh.texture_id);
		if texture.is_none() {
			return;
		}
		let texture = texture.unwrap();

		let push_constants = glm::vec2(width, height);
		
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
                .map(|pixel| pixel.to_array())
                .flatten()
                .collect(),
            egui::ImageData::Font(image) => image
                .srgba_pixels(1.0)
                .map(|pixel| pixel.to_array())
                .flatten()
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
                if let None = original_texture {
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
}
