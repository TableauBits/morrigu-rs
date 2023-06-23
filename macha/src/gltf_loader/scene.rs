use morrigu::{
    components::transform::Transform, renderer::Renderer, shader::Shader, texture::Texture,
    utils::ThreadSafeRef,
};

pub type Vertex = morrigu::sample_vertex::TexturedVertex;
pub type Material = morrigu::material::Material<Vertex>;
pub type Mesh = morrigu::mesh::Mesh<Vertex>;
pub type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

pub struct Scene {
    pub default_material: ThreadSafeRef<Material>,
    pub pbr_shader: ThreadSafeRef<Shader>,

    pub images: Vec<ThreadSafeRef<Texture>>,
    pub meshes: Vec<ThreadSafeRef<Mesh>>,
    pub materials: Vec<ThreadSafeRef<Material>>,
    pub mesh_renderings: Vec<ThreadSafeRef<MeshRendering>>,
    pub transforms: Vec<Transform>,
}

impl Scene {
    pub fn destroy(&mut self, renderer: &mut Renderer) {
        for mesh_rendering in &self.mesh_renderings {
            let mut mesh_rendering = mesh_rendering.lock();

            mesh_rendering
                .descriptor_resources
                .uniform_buffers
                .values()
                .for_each(|uniform_buffer| {
                    uniform_buffer
                        .lock()
                        .destroy(&renderer.device, &mut renderer.allocator())
                });
            mesh_rendering.destroy(renderer);
        }

        for material in &self.materials {
            let mut material = material.lock();

            material
                .descriptor_resources
                .uniform_buffers
                .values()
                .for_each(|uniform_buffer| {
                    uniform_buffer
                        .lock()
                        .destroy(&renderer.device, &mut renderer.allocator())
                });
            material.destroy(renderer);
        }

        for mesh in &self.meshes {
            mesh.lock().destroy(renderer);
        }

        for image in &self.images {
            image.lock().destroy(renderer);
        }

        self.pbr_shader.lock().destroy(&renderer.device);
        let mut default_material = self.default_material.lock();
        default_material.shader_ref.lock().destroy(&renderer.device);
        default_material.destroy(renderer);
    }
}
