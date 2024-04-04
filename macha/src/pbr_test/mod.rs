use std::path::Path;

use morrigu::{
    application::{ApplicationState, BuildableApplicationState},
    components::transform::Transform,
    descriptor_resources::DescriptorResources,
    math_types::{EulerRot, Quat},
    shader::Shader,
    utils::ThreadSafeRef,
};

use crate::utils::{
    camera::MachaCamera,
    ui::{draw_debug_utils, draw_state_switcher, SwitchableStates},
};

type Vertex = morrigu::vertices::textured::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

pub struct PBRState {
    camera: MachaCamera,

    shader_ref: ThreadSafeRef<Shader>,
    mesh_ref: ThreadSafeRef<Mesh>,
    mesh_renderings_ref: Vec<ThreadSafeRef<MeshRendering>>,

    desired_state: SwitchableStates,
}

impl BuildableApplicationState<()> for PBRState {
    fn build(context: &mut morrigu::application::StateContext, _data: ()) -> Self {
        let shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/flat/flat.vert"),
            include_bytes!("shaders/gen/flat/flat.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create shader");

        let material_ref = Material::builder()
            .build(&shader_ref, DescriptorResources::empty(), context.renderer)
            .expect("Failed to create material");

        let mesh_ref = Vertex::load_model_from_path_ply(
            Path::new("assets/meshes/sphere.ply"),
            context.renderer,
        )
        .expect("Failed to create mesh");

        let mesh_rendering_ref = MeshRendering::new(
            &mesh_ref,
            &material_ref,
            DescriptorResources {
                uniform_buffers: [morrigu::components::mesh_rendering::default_ubo_bindings(
                    context.renderer,
                )
                .unwrap()]
                .into(),
                ..Default::default()
            },
            context.renderer,
        )
        .expect("Failed to create mesh rendering");

        Self {
            camera: MachaCamera::new(morrigu::components::camera::Camera::default()),

            shader_ref,
            mesh_ref,
            mesh_renderings_ref: vec![mesh_rendering_ref],

            desired_state: SwitchableStates::PBRTest,
        }
    }
}

impl ApplicationState for PBRState {
    fn on_attach(&mut self, context: &mut morrigu::application::StateContext) {
        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_systems(morrigu::systems::mesh_renderer::render_meshes::<Vertex>);
        });

        let res = context.renderer.window_resolution();
        self.camera.on_resize(res.0, res.1);

        let mut transform = Transform::default();
        transform.rotate(&Quat::from_euler(
            EulerRot::XYZ,
            f32::to_radians(-90.0),
            0.0,
            0.0,
        ));
        self.camera.set_focal_point(transform.translation());
        self.camera.set_distance(7.0);

        context
            .ecs_manager
            .world
            .spawn((transform, self.mesh_renderings_ref[0].clone()));
    }

    fn on_drop(&mut self, context: &mut morrigu::application::StateContext) {
        self.mesh_renderings_ref[0]
            .lock()
            .descriptor_resources
            .uniform_buffers
            .get(&0)
            .unwrap()
            .lock()
            .destroy(&context.renderer.device, &mut context.renderer.allocator());
        self.mesh_renderings_ref[0].lock().destroy(context.renderer);
        self.mesh_ref.lock().destroy(context.renderer);
        self.mesh_renderings_ref[0]
            .lock()
            .material_ref
            .lock()
            .destroy(context.renderer);
        self.shader_ref.lock().destroy(&context.renderer.device);
    }

    fn on_update(
        &mut self,
        dt: std::time::Duration,
        context: &mut morrigu::application::StateContext,
    ) {
        self.camera.on_update(dt, context.window_input_state);
        context
            .ecs_manager
            .world
            .insert_resource(self.camera.mrg_camera);
    }

    fn on_update_egui(
        &mut self,
        dt: std::time::Duration,
        context: &mut morrigu::application::EguiUpdateContext,
    ) {
        draw_state_switcher(context.egui_context, &mut self.desired_state);
        draw_debug_utils(context.egui_context, dt);
    }

    fn on_event(
        &mut self,
        _event: morrigu::application::Event<()>,
        _context: &mut morrigu::application::StateContext,
    ) {
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
            SwitchableStates::RTTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::rt_test::RayTracerState::build(context, ()),
            )),
            SwitchableStates::PBRTest => morrigu::application::StateFlow::Continue,
        }
    }
}
