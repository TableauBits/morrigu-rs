use bevy_ecs::{entity::Entity, schedule::SystemStage};
use morrigu::{
    application::{event, ApplicationBuilder, ApplicationState, StateContext},
    components::{
        camera::{Camera, PerspectiveData},
        transform::{Axis, Transform},
    },
    shader::Shader,
    systems::mesh_renderer,
    texture::Texture,
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
    texture_ref: Option<ThreadSafeRef<Texture>>,
    rock: Option<Entity>,
}

impl MachaState {
    pub fn new() -> Self {
        Self {
            shader_ref: None,
            material_ref: None,
            mesh_ref: None,
            mesh_rendering_ref: None,
            texture_ref: None,
            rock: None,
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
            Shader::from_spirv_u8(
                include_bytes!("../assets/gen/shaders/test/test.vert"),
                include_bytes!("../assets/gen/shaders/test/test.frag"),
                &context.renderer.device,
            )
            .expect("Failed to create shader"),
        );
        self.material_ref = Some(
            Material::builder()
                .build(self.shader_ref.as_ref().unwrap(), context.renderer)
                .expect("Failed to create material"),
        );
        self.mesh_ref = Some(
            Vertex::load_model_from_path(Path::new("assets/meshes/rock.obj"), context.renderer)
                .expect("Failed to create mesh"),
        );

        let texture_ref =
            Texture::from_path(Path::new("assets/textures/rock.jpg"), context.renderer)
                .expect("Failed to load texture");
        self.texture_ref = Some(texture_ref.clone());

        let mesh_rendering_ref = MeshRendering::new(
            self.mesh_ref.as_ref().unwrap(),
            self.material_ref.as_ref().unwrap(),
            context.renderer,
        )
        .expect("Failed to create mesh rendering");
        mesh_rendering_ref
            .lock()
            .bind_texture(1, &texture_ref, context.renderer)
            .expect("Failed to bind texture")
            .lock()
            .destroy(context.renderer);
        self.mesh_rendering_ref = Some(mesh_rendering_ref.clone());

        let mut tranform = Transform::default();
        tranform
            .translate(&glm::vec3(0.0, -3.0, -15.0))
            .scale(&glm::vec3(0.02, 0.02, 0.02));

        self.rock = Some(
            context
                .ecs_manager
                .world
                .spawn()
                .insert(tranform)
                .insert(mesh_rendering_ref)
                .id(),
        );

        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_stage(
                "render meshes",
                SystemStage::parallel().with_system(mesh_renderer::render_meshes::<Vertex>),
            );
        })
    }

    fn on_update(&mut self, dt: std::time::Duration, context: &mut StateContext) {
        context
            .ecs_manager
            .world
            .get_entity_mut(self.rock.unwrap())
            .unwrap()
            .get_mut::<Transform>()
            .unwrap()
            .rotate(f32::to_radians(25.0) * dt.as_secs_f32(), Axis::Y);
    }

    fn on_update_imgui(&mut self, ui: &mut imgui::Ui, _context: &mut StateContext) {
        ui.show_demo_window(&mut true);
    }

    fn on_event(&mut self, _event: event::Event<()>, _context: &mut StateContext) {}

    fn on_drop(&mut self, context: &mut StateContext) {
        self.texture_ref
            .as_ref()
            .unwrap()
            .lock()
            .destroy(context.renderer);
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
