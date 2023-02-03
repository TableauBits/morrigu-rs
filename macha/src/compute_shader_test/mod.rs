use morrigu::{
    application::{ApplicationState, BuildableApplicationState},
    texture::Texture,
    utils::ThreadSafeRef,
};

type Vertex = morrigu::sample_vertex::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

pub struct CSTState {
    display_texture: ThreadSafeRef<Texture>,

    rendering_material: ThreadSafeRef<Material>,
    mesh_rendering_ref: ThreadSafeRef<MeshRendering>,
}

impl BuildableApplicationState<()> for CSTState {
    fn build(context: &mut morrigu::application::StateContext, _: ()) -> Self {
        todo!();
    }
}

impl ApplicationState for CSTState {
    fn on_attach(&mut self, _context: &mut morrigu::application::StateContext) {}

    fn on_update(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::StateContext,
    ) {
    }

    fn on_update_egui(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::EguiUpdateContext,
    ) {
    }

    fn on_drop(&mut self, _context: &mut morrigu::application::StateContext) {}
}
