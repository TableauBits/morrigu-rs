use ply_rs::ply;
use thiserror::Error;

use crate::mesh::{MeshDataUploadError, UploadError};

pub mod simple;
pub mod textured;

// used by all (for now ?) vertex types for deserialization

#[derive(Error, Debug)]
pub enum VertexModelLoadingError {
    #[error("Loading of the OBJ file failed with error: {0}.")]
    OBJLoadError(#[from] tobj::LoadError),

    #[error("Uploading of the mesh data failed with error: {0}.")]
    MeshDatauploadFailed(#[from] MeshDataUploadError),

    #[error("Reading of model file failed with error: {0}.")]
    FileReadingError(#[from] std::io::Error),

    #[error("Uploading of the mesh data failed with error: {0}.")]
    BufferUploadFailed(#[from] UploadError),
}

pub(crate) struct Face {
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
