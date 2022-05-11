use crate::{
    components::{mesh_rendering::MeshRendering, transform::Transform},
    error::Error,
    material::Vertex,
};
use bevy_ecs::{entity::Entity, prelude::Query};

pub fn render_meshes<VertexType>(
    query: Query<(Entity, &Transform, &MeshRendering<VertexType>)>,
) -> Result<(), Error>
where
    VertexType: Vertex,
{
    Ok(())
}
