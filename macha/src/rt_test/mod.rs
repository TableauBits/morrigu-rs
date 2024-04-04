use morrigu::{
    application::{ApplicationState, BuildableApplicationState, EguiUpdateContext},
    components::ray_tracing::{mesh_rendering::MeshRendering, tlas::TLAS},
    utils::ThreadSafeRef,
    vertices::simple::SimpleVertex,
};

use crate::utils::ui::{draw_state_switcher, SwitchableStates};

pub struct RayTracerState {
    monkey_mr: ThreadSafeRef<MeshRendering<SimpleVertex>>,
    rock_mr: ThreadSafeRef<MeshRendering<SimpleVertex>>,
    tlas: ThreadSafeRef<TLAS>,

    desired_state: SwitchableStates,
}

impl BuildableApplicationState<()> for RayTracerState {
    fn build(context: &mut morrigu::application::StateContext, _: ()) -> Self {
        let monkey = SimpleVertex::load_model_from_path_obj(
            std::path::Path::new("assets/meshes/monkey.obj"),
            context.renderer,
        )
        .expect("Failed to load mesh");
        let monkey_mesh = MeshRendering::new(monkey, context.renderer)
            .expect("Failed to convert Mesh to ray tracing mesh");

        let rock = SimpleVertex::load_model_from_path_obj(
            std::path::Path::new("assets/meshes/monkey.obj"),
            context.renderer,
        )
        .expect("Failed to load mesh");
        let rock_mesh = MeshRendering::new(rock, context.renderer)
            .expect("Failed to convert Mesh to ray tracing mesh");

        let tlas = TLAS::new(
            &[
                *monkey_mesh.lock().tlas_instance(),
                *rock_mesh.lock().tlas_instance(),
            ],
            context.renderer,
        )
        .expect("Failed to build TLAS");
        Self {
            monkey_mr: monkey_mesh,
            rock_mr: rock_mesh,
            tlas,

            desired_state: SwitchableStates::RTTest,
        }
    }
}

impl ApplicationState for RayTracerState {
    fn on_attach(&mut self, _context: &mut morrigu::application::StateContext) {}

    fn on_drop(&mut self, context: &mut morrigu::application::StateContext) {
        self.tlas.lock().destroy(context.renderer);
        self.rock_mr.lock().destroy(context.renderer);
        self.monkey_mr.lock().destroy(context.renderer);

        self.rock_mr
            .lock()
            .mesh_ref
            .lock()
            .destroy(context.renderer);
        self.monkey_mr
            .lock()
            .mesh_ref
            .lock()
            .destroy(context.renderer);
    }

    fn on_update_egui(&mut self, _dt: std::time::Duration, context: &mut EguiUpdateContext) {
        draw_state_switcher(context.egui_context, &mut self.desired_state);
    }

    fn flow<'flow>(
        &mut self,
        context: &mut morrigu::application::StateContext,
    ) -> morrigu::application::StateFlow<'flow> {
        match self.desired_state {
            SwitchableStates::Editor => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::editor::MachaState::build(context, ()),
            )),
            SwitchableStates::GLTFLoader => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::gltf_loader::GLTFViewerState::build(context, ()),
            )),
            SwitchableStates::CSTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::compute_shader_test::CSTState::build(context, ()),
            )),
            SwitchableStates::RTTest => morrigu::application::StateFlow::Continue,
        }
    }
}
