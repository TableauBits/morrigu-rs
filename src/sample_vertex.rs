use ash::vk;

use crate::{
    error::Error,
    material::{Vertex, VertexInputDescription},
    mesh::{upload_index_buffer, upload_mesh_data, upload_vertex_buffer, Mesh},
    renderer::Renderer,
    utils::ThreadSafeRef, vector_type::{Vec3, Vec2},
};

use ply_rs::{parser, ply};

#[repr(C)]
pub struct TexturedVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub texture_coords: Vec2,
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

impl ply::PropertyAccess for TexturedVertex {
    fn new() -> Self {
        Self {
            position: Vec3::default(),
            normal: Vec3::default(),
            texture_coords: Vec2::default(),
        }
    }

    fn set_property(&mut self, key: String, property: ply::Property) {
        match (key.as_ref(), property) {
            ("x", ply::Property::Float(v)) => self.position.x = v,
            ("y", ply::Property::Float(v)) => self.position.y = v,
            ("z", ply::Property::Float(v)) => self.position.z = v,
            ("nx", ply::Property::Float(v)) => self.normal.x = v,
            ("ny", ply::Property::Float(v)) => self.normal.y = v,
            ("nz", ply::Property::Float(v)) => self.normal.z = v,
            ("s", ply::Property::Float(v)) => self.texture_coords.x = v,
            ("t", ply::Property::Float(v)) => self.texture_coords.y = v,
            (_, _) => (),
        }
    }
}

struct Face {
    indices: Vec<u32>,
}

impl ply::PropertyAccess for Face {
    fn new() -> Self {
        Self {
            indices: Vec::default(),
        }
    }

    #[allow(clippy::single_match)]
    fn set_property(&mut self, key: String, property: ply::Property) {
        match (key.as_ref(), property) {
            ("vertex_indices", ply::Property::ListUInt(v)) => self.indices = v,
            (_, _) => (),
        }
    }
}

impl TexturedVertex {
    pub fn load_model_from_path_obj(
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
            .map(|slice| Vec3::new(slice[0], slice[1], slice[2]))
            .collect::<Vec<Vec3>>();
        let normals = mesh
            .normals
            .chunks_exact(3)
            .map(|slice| Vec3::new(slice[0], slice[1], slice[2]))
            .collect::<Vec<Vec3>>();
        let texture_coordinates = mesh
            .texcoords
            .chunks_exact(2)
            .map(|slice| Vec2::new(slice[0], slice[1]))
            .collect::<Vec<Vec2>>();

        let mut vertices = Vec::with_capacity(positions.len());
        for index in 0..positions.len() {
            vertices.push(TexturedVertex {
                position: positions[index],
                normal: normals[index],
                texture_coords: texture_coordinates[index],
            });
        }

        let indices = mesh.indices.clone();

        let upload_result = upload_mesh_data(&vertices, &indices, renderer)?;

        Ok(ThreadSafeRef::new(Mesh::<Self> {
            vertices,
            indices: Some(indices),
            vertex_buffer: upload_result.vertex_buffer,
            index_buffer: Some(upload_result.index_buffer),
        }))
    }

    pub fn load_model_from_path_ply(
        path: &std::path::Path,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Mesh<Self>>, Error> {
        let file = std::fs::File::open(path)?;
        let mut file = std::io::BufReader::new(file);

        let vertex_parser = parser::Parser::<Self>::new();
        let face_parser = parser::Parser::<Face>::new();

        let header = vertex_parser.read_header(&mut file)?;

        let mut vertices = vec![];
        let mut faces = vec![];
        for (_, element) in &header.elements {
            #[allow(clippy::single_match)]
            match element.name.as_ref() {
                "vertex" => {
                    vertices =
                        vertex_parser.read_payload_for_element(&mut file, element, &header)?;
                }
                "face" => {
                    faces = face_parser.read_payload_for_element(&mut file, element, &header)?;
                }
                _ => (),
            }
        }

        let vertex_buffer = upload_vertex_buffer(&vertices, renderer)?;

        let mut indices = vec![];
        indices.reserve(faces.len() * 3);
        for face in faces {
            indices.extend(face.indices.iter());
        }
        let index_buffer = upload_index_buffer(&indices, renderer)?;

        Ok(ThreadSafeRef::new(Mesh::<Self> {
            vertices,
            indices: Some(indices),
            vertex_buffer,
            index_buffer: Some(index_buffer),
        }))
    }
}
