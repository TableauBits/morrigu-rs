use morrigu::{
    application::{ApplicationState, BuildableApplicationState},
    components::rt_mesh_rendering::RTMeshRendering,
    utils::ThreadSafeRef,
    vertices::simple::SimpleVertex,
    winit,
};

pub struct RayTracerState {
    rt_mesh: ThreadSafeRef<RTMeshRendering<SimpleVertex>>,
}

impl BuildableApplicationState<()> for RayTracerState {
    fn build(context: &mut morrigu::application::StateContext, _: ()) -> Self {
        let mesh = SimpleVertex::load_model_from_path_obj(
            std::path::Path::new("assets/meshes/monkey.obj"),
            context.renderer,
        )
        .expect("Failed to load mesh");
        let rt_mesh = RTMeshRendering::new(mesh, context.renderer)
            .expect("Failed to convert Mesh to ray tracing mesh");
        Self { rt_mesh }
    }
}

impl ApplicationState for RayTracerState {
    fn on_attach(&mut self, _context: &mut morrigu::application::StateContext) {}

    fn on_update(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::StateContext,
    ) {
    }

    fn after_systems(
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

    fn after_ui_systems(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::EguiUpdateContext,
    ) {
    }

    fn on_event(
        &mut self,
        _event: winit::event::Event<()>,
        _context: &mut morrigu::application::StateContext,
    ) {
    }

    fn on_drop(&mut self, _context: &mut morrigu::application::StateContext) {}
}
