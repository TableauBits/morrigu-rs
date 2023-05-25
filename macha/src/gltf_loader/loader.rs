use anyhow::Context;
use morrigu::{
    allocated_types::AllocatedBuffer,
    components::{mesh_rendering::default_descriptor_resources, transform::Transform},
    descriptor_resources::DescriptorResources,
    math_types::{Mat4, Vec3, Vec4},
    mesh::{upload_index_buffer, upload_vertex_buffer, Mesh},
    renderer::Renderer,
    shader::Shader,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
};
use std::{hint::black_box, iter::zip, path::Path};

use super::scene::{Material, MeshRendering, Scene, Vertex};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LightData {
    pub light_direction: Vec4,
    pub light_color: Vec4,
    pub ambient_light_color: Vec3,
    pub ambient_light_intensity: f32,

    pub camera_position: Vec4,
}

unsafe impl bytemuck::Zeroable for LightData {}
unsafe impl bytemuck::Pod for LightData {}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PBRData {
    base_color_factor: Vec4,
    metallic_factor: f32,
    roughness_factor: f32,

    alpha_cutoff: f32,

    _padding: f32,
}

unsafe impl bytemuck::Zeroable for PBRData {}
unsafe impl bytemuck::Pod for PBRData {}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MapPresenceInfo {
    has_base_color_map: u32,
    has_normal_map: u32,
    has_metal_roughness_map: u32,

    _padding: u32,
}

unsafe impl bytemuck::Zeroable for MapPresenceInfo {}
unsafe impl bytemuck::Pod for MapPresenceInfo {}

fn convert_texture_format(gltf_format: gltf::image::Format) -> TextureFormat {
    match gltf_format {
        gltf::image::Format::R8 => todo!(),
        gltf::image::Format::R8G8 => todo!(),
        gltf::image::Format::R8G8B8 => todo!(),
        gltf::image::Format::R8G8B8A8 => TextureFormat::RGBA8_UNORM,
        gltf::image::Format::R16 => todo!(),
        gltf::image::Format::R16G16 => todo!(),
        gltf::image::Format::R16G16B16 => todo!(),
        gltf::image::Format::R16G16B16A16 => todo!(),
        gltf::image::Format::R32G32B32FLOAT => todo!(),
        gltf::image::Format::R32G32B32A32FLOAT => todo!(),
    }
}

pub fn load_node(
    current_node: gltf::Node,
    parent_transform: Transform,
) -> (Vec<Mat4>, Vec<MeshRendering>) {
    let transforms = vec![];
    let mesh_renderings = vec![];

    let diff_transform = match current_node.transform() {
        gltf::scene::Transform::Matrix { matrix } => Transform::from_matrix(matrix.into()),
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => {
            let mut initial = Transform::default();
            initial.set_position(&translation.into());
            initial.set_rotation(&rotation.into());
            initial.set_scale(&scale.into());

            initial
        }
    };
    let new_transform = parent_transform * diff_transform;

    (transforms, mesh_renderings)
}

pub fn load_gltf(
    path: &Path,
    pbr_shader: ThreadSafeRef<Shader>,
    default_texture: ThreadSafeRef<Texture>,
    default_material: ThreadSafeRef<Material>,
    renderer: &mut Renderer,
) -> anyhow::Result<Scene> {
    let (document, buffers, images) = gltf::import(path)?;

    let images = images
        .into_iter()
        .map(|data| {
            Texture::builder()
                .with_format(convert_texture_format(data.format))
                .build_from_data(&data.pixels, data.width, data.height, renderer)
        })
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to build textures")?;

    let mut materials = vec![];

    for material in document.materials() {
        let metallic_data = material.pbr_metallic_roughness();

        let params = PBRData {
            base_color_factor: metallic_data.base_color_factor().into(),
            metallic_factor: metallic_data.metallic_factor(),
            roughness_factor: metallic_data.roughness_factor(),
            alpha_cutoff: material.alpha_cutoff().unwrap_or(-1.0),
            _padding: 0.0,
        };

        let base_color_map = metallic_data.base_color_texture();
        let normal_map = material.normal_texture();
        let metal_roughness_map = metallic_data.metallic_roughness_texture();
        let map_presence_info = black_box(MapPresenceInfo {
            has_base_color_map: base_color_map.is_some().into(),
            has_normal_map: normal_map.is_some().into(),
            has_metal_roughness_map: metal_roughness_map.is_some().into(),
            _padding: 0,
        });

        let new_material = Material::builder()
            .build::<Vertex>(
                &pbr_shader,
                DescriptorResources {
                    uniform_buffers: [
                        (
                            0,
                            ThreadSafeRef::new(
                                AllocatedBuffer::builder(
                                    std::mem::size_of::<LightData>().try_into()?,
                                )
                                .build(renderer)
                                .context("Failed to build LightData buffer")?,
                            ),
                        ),
                        (
                            1,
                            ThreadSafeRef::new(
                                AllocatedBuffer::builder(
                                    std::mem::size_of::<PBRData>().try_into()?,
                                )
                                .build_with_data(params, renderer)
                                .context("Failed to build PBRData buffer")?,
                            ),
                        ),
                        (
                            2,
                            ThreadSafeRef::new(
                                AllocatedBuffer::builder(
                                    std::mem::size_of::<MapPresenceInfo>().try_into()?,
                                )
                                .build_with_data(map_presence_info, renderer)
                                .context("Failed to create map presence info buffer")?,
                            ),
                        ),
                    ]
                    .into(),
                    sampled_images: [
                        (
                            3,
                            if let Some(color_map_info) = base_color_map {
                                images[color_map_info.texture().index()].clone()
                            } else {
                                default_texture.clone()
                            },
                        ),
                        (
                            4,
                            if let Some(normal_map_info) = normal_map {
                                images[normal_map_info.texture().index()].clone()
                            } else {
                                default_texture.clone()
                            },
                        ),
                        (
                            5,
                            if let Some(mr_map) = metal_roughness_map {
                                images[mr_map.texture().index()].clone()
                            } else {
                                default_texture.clone()
                            },
                        ),
                    ]
                    .into(),
                    ..Default::default()
                },
                renderer,
            )
            .context("Failed to create material")?;

        materials.push(new_material);
    }

    let mut meshes = vec![];
    let mut mesh_renderings = vec![];
    let mut transforms = vec![];

    let scene = match document.default_scene() {
        Some(default_scene) => default_scene,
        None => document.scenes().next().context("No scene in gltf file")?,
    };

    for node in scene.nodes() {
        if let Some(mesh) = node.mesh() {
            // We are considering each primitive as a mesh
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
                let positions = reader
                    .read_positions()
                    .context("primitive must have POSITION attribute")?;
                let normals = reader
                    .read_normals()
                    .context("primitive must have NORMAL attribute")?;
                let uvs = reader
                    .read_tex_coords(0)
                    .context("primitive must have TEXCOORD0 attribute")?
                    .into_f32();

                let vertices = zip(zip(positions, normals), uvs)
                    .map(|((positions, normals), uvs)| Vertex {
                        position: positions.into(),
                        normal: normals.into(),
                        texture_coords: uvs.into(),
                    })
                    .collect::<Vec<_>>();

                let vertex_buffer = upload_vertex_buffer(&vertices, renderer)?;

                let (index_buffer, indices) = match reader.read_indices() {
                    Some(indices) => {
                        let indices = indices.into_u32().collect::<Vec<_>>();
                        (
                            Some(upload_index_buffer(&indices, renderer)?),
                            Some(indices),
                        )
                    }
                    None => (None, None),
                };

                let new_mesh_ref = ThreadSafeRef::new(Mesh {
                    vertices,
                    indices,
                    vertex_buffer,
                    index_buffer,
                });
                meshes.push(new_mesh_ref.clone());

                let material_ref = match primitive.material().index() {
                    Some(index) => materials[index].clone(),
                    None => default_material.clone(),
                };
                mesh_renderings.push(MeshRendering::new(
                    &new_mesh_ref,
                    &material_ref,
                    default_descriptor_resources(renderer)?,
                    renderer,
                )?);

                // @TODO(Ithyx): actually make this work??
                transforms.push(Transform::default());
            }
        }
    }

    Ok(Scene {
        default_material,
        pbr_shader,
        images,
        meshes,
        materials,
        mesh_renderings,
        transforms,
    })
}
