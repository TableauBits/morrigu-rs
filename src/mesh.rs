use crate::{allocated_types::AllocatedBuffer, material::Vertex, renderer::Renderer};

pub struct Mesh<VertexType>
where
    VertexType: Vertex,
{
    pub(crate) vertices: Vec<VertexType>,
    pub(crate) indices: Vec<u32>,
    pub(crate) vertex_buffer: AllocatedBuffer,
}

impl<VertexType> Mesh<VertexType>
where
    VertexType: Vertex,
{
    pub fn destroy(self, renderer: &mut Renderer) {
        self.vertex_buffer
            .destroy(&renderer.device, renderer.allocator.as_mut().unwrap());
    }
}
