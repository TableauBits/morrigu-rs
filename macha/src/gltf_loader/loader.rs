use anyhow::Context;
use gltf::buffer::Data;
use morrigu::{
    allocated_types::AllocatedBuffer,
    components::{mesh_rendering::default_descriptor_resources, transform::Transform},
    descriptor_resources::DescriptorResources,
    math_types::{Mat4, Quat, Vec3, Vec4},
    mesh::{upload_index_buffer, upload_vertex_buffer},
    renderer::Renderer,
    shader::Shader,
    texture::Texture,
    utils::ThreadSafeRef,
};
use std::{hint::black_box, iter::zip, path::Path};

use super::scene::{Material, Mesh, MeshRendering, Scene, Vertex};

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

#[derive(Debug, Default)]
pub struct LoadData {
    pub meshes: Vec<ThreadSafeRef<Mesh>>,
    pub mesh_renderings: Vec<ThreadSafeRef<MeshRendering>>,
    pub transforms: Vec<Transform>,
}

fn convert_transform(value: gltf::scene::Transform) -> Transform {
    match value {
        gltf::scene::Transform::Matrix { matrix } => Mat4::from_cols_array_2d(&matrix).into(),
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => Transform::from_trs(
            &translation.into(),
            &Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
            &scale.into(),
        ),
    }
}

pub fn load_node(
    current_node: &gltf::Node,
    parent_transform: Transform,
    materials: &[ThreadSafeRef<Material>],
    buffers: &[Data],
    default_material: &ThreadSafeRef<Material>,
    renderer: &mut Renderer,
) -> anyhow::Result<LoadData> {
    let mut load_data = LoadData::default();

    let diff_transform = convert_transform(current_node.transform());
    let current_transform = parent_transform * diff_transform;
    if let Some(mesh) = current_node.mesh() {
        // We are considering each primitive as a mesh
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let positions = reader
                .read_positions()
                .context("primitive must have POSITION attribute")?;
            let normals = reader
                .read_normals()
                .context("primitive must have NORMAL attribute")?;
            let uvs: Box<dyn Iterator<Item = [f32; 2]>> = match reader.read_tex_coords(0) {
                Some(reader) => Box::new(reader.into_f32()),
                None => Box::new(std::iter::repeat([0.0, 0.0])),
            };

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
            load_data.meshes.push(new_mesh_ref.clone());

            let material_ref = match primitive.material().index() {
                Some(index) => materials[index].clone(),
                None => default_material.clone(),
            };
            load_data.mesh_renderings.push(MeshRendering::new(
                &new_mesh_ref,
                &material_ref,
                default_descriptor_resources(renderer)?,
                renderer,
            )?);

            load_data.transforms.push(current_transform.clone());
        }
    }

    for child in current_node.children() {
        let mut child_data = load_node(
            &child,
            current_transform.clone(),
            materials,
            buffers,
            default_material,
            renderer,
        )?;
        load_data.meshes.append(&mut child_data.meshes);
        load_data
            .mesh_renderings
            .append(&mut child_data.mesh_renderings);
        load_data.transforms.append(&mut child_data.transforms);
    }

    Ok(load_data)
}

pub fn load_gltf(
    path: &Path,
    transform: Transform,
    pbr_shader: ThreadSafeRef<Shader>,
    default_texture: ThreadSafeRef<Texture>,
    default_material: ThreadSafeRef<Material>,
    renderer: &mut Renderer,
) -> anyhow::Result<Scene> {
    let (document, buffers, images) = gltf::import(path)?;

    let images = images
        .into_iter()
        .map(|image| {
            let image = image
                .convert_format(gltf::image::Format::R8G8B8A8)
                .context("Failed to convert GLTF image to RGAB8")?;
            Texture::builder()
                .build_from_data(&image.pixels, image.width, image.height, renderer)
                .context("Failed to create texture form GTLF data")
        })
        .collect::<anyhow::Result<Vec<_>, _>>()
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

    let scene = match document.default_scene() {
        Some(default_scene) => default_scene,
        None => document.scenes().next().context("No scene in gltf file")?,
    };

    let mut load_data = LoadData::default();
    for root_node in scene.nodes() {
        let initial_transform = transform.clone() * convert_transform(root_node.transform());
        let mut current_load_data = load_node(
            &root_node,
            initial_transform,
            &materials,
            &buffers,
            &default_material,
            renderer,
        )?;

        load_data.meshes.append(&mut current_load_data.meshes);
        load_data
            .mesh_renderings
            .append(&mut current_load_data.mesh_renderings);
        load_data
            .transforms
            .append(&mut current_load_data.transforms);
    }

    Ok(Scene {
        default_material,
        pbr_shader,
        images,
        meshes: load_data.meshes,
        materials,
        mesh_renderings: load_data.mesh_renderings,
        transforms: load_data.transforms,
    })
}
