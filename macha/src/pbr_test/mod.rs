use std::path::Path;

use morrigu::{
    application::{ApplicationState, BuildableApplicationState},
    components::transform::Transform,
    descriptor_resources::DescriptorResources,
    math_types::{EulerRot, Quat, Vec2, Vec3},
    shader::Shader,
    utils::ThreadSafeRef,
};

use crate::utils::{
    camera::MachaCamera,
    startup_state::SwitchableStates,
    ui::{draw_debug_utils, draw_state_switcher},
};

type Vertex = morrigu::vertices::textured::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

pub struct PBRState {
    camera: MachaCamera,

    flat_shader_ref: ThreadSafeRef<Shader>,
    pbr_shader_ref: ThreadSafeRef<Shader>,

    mesh_ref: ThreadSafeRef<Mesh>,
    mesh_renderings_ref: Vec<ThreadSafeRef<MeshRendering>>,

    desired_state: SwitchableStates,
}

impl BuildableApplicationState<()> for PBRState {
    fn build(context: &mut morrigu::application::StateContext, _data: ()) -> Self {
        let flat_shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/flat/flat.vert"),
            include_bytes!("shaders/gen/flat/flat.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create flat shader");
        let pbr_shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/pbr/pbr.vert"),
            include_bytes!("shaders/gen/pbr/pbr.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create pbr shader");

        let mesh_ref = Vertex::load_model_from_path_ply(
            Path::new("assets/meshes/sphere.ply"),
            context.renderer,
        )
        .expect("Failed to create mesh");

        let mut mesh_renderings = vec![];

        {
            // sample flat material
            let flat_material_ref = Material::builder()
                .build(
                    &flat_shader_ref,
                    DescriptorResources::empty(),
                    context.renderer,
                )
                .expect("Failed to create material");

            let mesh_rendering_ref = MeshRendering::new(
                &mesh_ref,
                &flat_material_ref,
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

            mesh_renderings.push(mesh_rendering_ref);
        }

        {
            // some basic diffuse materials to start
            let diffuse_material_ref = Material::builder()
                .build(
                    &pbr_shader_ref,
                    DescriptorResources::empty(),
                    context.renderer,
                )
                .expect("Failed to create material");

            let mesh_rendering_ref = MeshRendering::new(
                &mesh_ref,
                &diffuse_material_ref,
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

            mesh_renderings.push(mesh_rendering_ref);
        }

        let camera = morrigu::components::camera::Camera::builder().build(
            morrigu::components::camera::Projection::Orthographic(
                morrigu::components::camera::OrthographicData {
                    scale: 15.0,
                    near_plane: 0.00001,
                    far_plane: 100.0,
                },
            ),
            &Vec2::new(1280.0, 720.0),
        );

        Self {
            camera: MachaCamera::new(camera),

            flat_shader_ref,
            pbr_shader_ref,

            mesh_ref,
            mesh_renderings_ref: mesh_renderings,

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

        for (i, mrr) in self.mesh_renderings_ref.iter().enumerate() {
            let mut transform = transform.clone();
            transform.translate(&Vec3::new(
                -10.0 + (((20 / (self.mesh_renderings_ref.len() - 1)) * i) as f32),
                0.0,
                0.0,
            ));

            context.ecs_manager.world.spawn((transform, mrr.clone()));
        }
    }

    fn on_drop(&mut self, context: &mut morrigu::application::StateContext) {
        for mrr in &mut self.mesh_renderings_ref {
            mrr.lock()
                .descriptor_resources
                .uniform_buffers
                .get(&0)
                .unwrap()
                .lock()
                .destroy(&context.renderer.device, &mut context.renderer.allocator());
            mrr.lock().destroy(context.renderer);
            mrr.lock().material_ref.lock().destroy(context.renderer);
        }
        self.mesh_ref.lock().destroy(context.renderer);

        self.pbr_shader_ref.lock().destroy(&context.renderer.device);
        self.flat_shader_ref
            .lock()
            .destroy(&context.renderer.device);
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
        event: morrigu::application::Event<()>,
        _context: &mut morrigu::application::StateContext,
    ) {
        self.camera.on_event(&event);
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

            #[cfg(feature = "ray_tracing")]
            SwitchableStates::RTTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::rt_test::RayTracerState::build(context, ()),
            )),
            SwitchableStates::PBRTest => morrigu::application::StateFlow::Continue,
        }
    }
}
