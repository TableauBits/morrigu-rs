use bevy_ecs::{entity::Entity, schedule::SystemStage};
use morrigu::{
    application::{event, ApplicationBuilder, ApplicationState, StateContext},
    components::{
        camera::{Camera, PerspectiveData},
        transform::Transform,
    },
    shader::Shader,
    systems::mesh_renderer,
    utils::ThreadSafeRef,
};
use nalgebra_glm as glm;

use std::path::Path;

type Vertex = morrigu::sample_vertex::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

struct MachaState {
    shader_ref: Option<ThreadSafeRef<Shader>>,
    material_ref: Option<ThreadSafeRef<Material>>,
    mesh_ref: Option<ThreadSafeRef<Mesh>>,
    mesh_rendering_ref: Option<ThreadSafeRef<MeshRendering>>,
    monkey: Option<Entity>,
}

impl MachaState {
    pub fn new() -> Self {
        Self {
            shader_ref: None,
            material_ref: None,
            mesh_ref: None,
            mesh_rendering_ref: None,
            monkey: None,
        }
    }
}

impl ApplicationState for MachaState {
    fn on_attach(&mut self, context: &mut StateContext) {
        let camera = Camera::builder().build(
            morrigu::components::camera::Projection::Perspective(PerspectiveData {
                horizontal_fov: f32::to_radians(90.0),
                near_plane: 0.001,
                far_plane: 1000.0,
            }),
            16.0 / 9.0,
        );
        context.ecs_manager.world.insert_resource(camera);

        self.shader_ref = Some(
            Shader::from_path(
                Path::new("assets/gen/shaders/test/test.vert"),
                Path::new("assets/gen/shaders/test/test.frag"),
                &context.renderer.device,
            )
            .expect("Failed to create shader"),
        );
        self.material_ref = Some(
            Material::builder()
                .build(&self.shader_ref.as_ref().unwrap(), context.renderer)
                .expect("Failed to create material"),
        );
        self.mesh_ref = Some(
            Vertex::load_model_from_path(Path::new("assets/meshes/monkey.obj"), context.renderer)
                .expect("Failed to create mesh"),
        );

        let mesh_rendering_ref = MeshRendering::new(
            &self.mesh_ref.as_ref().unwrap(),
            &self.material_ref.as_ref().unwrap(),
            context.renderer,
        )
        .expect("Failed to create mesh rendering");
        self.mesh_rendering_ref = Some(mesh_rendering_ref.clone());

        let mut monkey_tranform = Transform::default().clone();
        monkey_tranform.translate(&glm::vec3(-5.0, 0.0, 0.0));

        self.monkey = Some(
            context
                .ecs_manager
                .world
                .spawn()
                .insert(monkey_tranform)
                .insert(mesh_rendering_ref.clone())
                .id(),
        );

        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_stage(
                "render meshes",
                SystemStage::parallel().with_system(mesh_renderer::render_meshes::<Vertex>),
            );
        })
    }

    fn on_update(&mut self, dt: std::time::Duration, context: &mut StateContext) {}

    fn on_event(&mut self, event: event::Event<()>, context: &mut StateContext) {}

    fn on_drop(&mut self, context: &mut StateContext) {
        self.mesh_rendering_ref
            .as_ref()
            .unwrap()
            .lock()
            .destroy(context.renderer);
        self.mesh_ref
            .as_ref()
            .unwrap()
            .lock()
            .destroy(context.renderer);
        self.material_ref
            .as_ref()
            .unwrap()
            .lock()
            .destroy(context.renderer);
        self.shader_ref
            .as_ref()
            .unwrap()
            .lock()
            .destroy(&context.renderer.device);
    }
}

fn init_logging() {
    #[cfg(debug_assertions)]
    let log_level = ("trace", flexi_logger::Duplicate::Debug);
    #[cfg(not(debug_assertions))]
    let log_level = ("info", flexi_logger::Duplicate::Info);

    let file_spec = flexi_logger::FileSpec::default().suppress_timestamp();

    let _logger = flexi_logger::Logger::try_with_env_or_str(log_level.0)
        .expect("Failed to setup logging")
        .log_to_file(file_spec)
        .write_mode(flexi_logger::WriteMode::BufferAndFlush)
        .duplicate_to_stdout(log_level.1)
        .set_palette("b9;3;2;8;7".to_owned())
        .start()
        .expect("Failed to build logger");
}

fn main() {
    init_logging();

    let mut state = MachaState::new();
    ApplicationBuilder::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .build_and_run(&mut state);
}
