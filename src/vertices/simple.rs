use ash::vk;
use ply_rs::{parser, ply};

use crate::{
    material::{Vertex, VertexInputDescription},
    math_types::Vec3,
    mesh::{upload_index_buffer, upload_mesh_data, upload_vertex_buffer, Mesh},
    renderer::Renderer,
    utils::ThreadSafeRef,
};

use super::{Face, VertexModelLoadingError};

#[repr(C)]
#[derive(Debug)]
pub struct SimpleVertex {
    pub position: Vec3,
}

impl Vertex for SimpleVertex {
    fn vertex_input_description() -> VertexInputDescription {
        let main_binding = vk::VertexInputBindingDescription::builder()
            .binding(0)
            .stride(
                std::mem::size_of::<SimpleVertex>()
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
                memoffset::offset_of!(SimpleVertex, position)
                    .try_into()
                    .expect("Unsupported architecture"),
            )
            .build();

        VertexInputDescription {
            bindings: vec![main_binding],
            attributes: vec![position],
        }
    }
}

impl ply::PropertyAccess for SimpleVertex {
    fn new() -> Self {
        Self {
            position: Vec3::default(),
        }
    }

    fn set_property(&mut self, key: String, property: ply::Property) {
        match (key.as_ref(), property) {
            ("x", ply::Property::Float(v)) => self.position.x = v,
            ("y", ply::Property::Float(v)) => self.position.y = v,
            ("z", ply::Property::Float(v)) => self.position.z = v,
            (_, _) => (),
        }
    }
}

impl SimpleVertex {
    pub fn load_model_from_path_obj(
        path: &std::path::Path,
        renderer: &mut Renderer,
    ) -> Result<ThreadSafeRef<Mesh<Self>>, VertexModelLoadingError> {
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

        let mut vertices = Vec::with_capacity(positions.len());
        for position in positions {
            vertices.push(SimpleVertex { position });
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
    ) -> Result<ThreadSafeRef<Mesh<Self>>, VertexModelLoadingError> {
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
