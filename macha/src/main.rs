use bevy_ecs::{entity::Entity, schedule::SystemStage};
use morrigu::{
    application::{ApplicationBuilder, ApplicationState, StateContext},
    components::{
        camera::{Camera, PerspectiveData},
        transform::{Axis, Transform},
    },
    shader::Shader,
    systems::mesh_renderer,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
};
use nalgebra_glm as glm;

use std::path::Path;

type Vertex = morrigu::sample_vertex::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

#[derive(Copy, Clone)]
#[repr(C)]
struct ShaderOptions {
    pub flow_speed: f32,
    pub flow_intensity: f32,
}

struct MachaState {
    shader_ref: Option<ThreadSafeRef<Shader>>,
    material_ref: Option<ThreadSafeRef<Material>>,
    mesh_ref: Option<ThreadSafeRef<Mesh>>,
    mesh_rendering_ref: Option<ThreadSafeRef<MeshRendering>>,
    texture_ref: Option<ThreadSafeRef<Texture>>,
    flowmap_ref: Option<ThreadSafeRef<Texture>>,
    gradient_ref: Option<ThreadSafeRef<Texture>>,
    rock: Option<Entity>,

    shader_options: ShaderOptions,
}

impl MachaState {
    pub fn new() -> Self {
        Self {
            shader_ref: None,
            material_ref: None,
            mesh_ref: None,
            mesh_rendering_ref: None,
            texture_ref: None,
            flowmap_ref: None,
            gradient_ref: None,
            rock: None,

            shader_options: ShaderOptions {
                flow_speed: 0.1,
                flow_intensity: 0.1,
            },
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
            Vertex::load_model_from_path_ply(
                Path::new("assets/meshes/sphere.ply"),
                context.renderer,
            )
            .expect("Failed to create mesh"),
        );

        let texture_ref = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .build_from_path(
                Path::new("assets/textures/jupiter_base.png"),
                context.renderer,
            )
            .expect("Failed to load texture");
        self.texture_ref = Some(texture_ref.clone());
        let flowmap_ref = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .build_from_path(
                Path::new("assets/textures/jupiter_flowmap.png"),
                context.renderer,
            )
            .expect("Failed to load flowmap texture");
        self.flowmap_ref = Some(flowmap_ref.clone());
        let gradient_ref = Texture::builder()
            .build_from_path(
                Path::new("assets/textures/jupiter_gradient.png"),
                context.renderer,
            )
            .expect("Failed to load gradient texture");
        self.gradient_ref = Some(gradient_ref.clone());

        let mesh_rendering_ref = MeshRendering::new(
            self.mesh_ref.as_ref().unwrap(),
            self.material_ref.as_ref().unwrap(),
            context.renderer,
        )
        .expect("Failed to create mesh rendering");
        mesh_rendering_ref
            .lock()
            .bind_texture(1, &texture_ref, context.renderer)
            .expect("Failed to bind base texture")
            .lock()
            .destroy(context.renderer);
        mesh_rendering_ref
            .lock()
            .bind_texture(2, &flowmap_ref, context.renderer)
            .expect("Failed to bind flowmap texture")
            .lock()
            .destroy(context.renderer);
        mesh_rendering_ref
            .lock()
            .bind_texture(3, &gradient_ref, context.renderer)
            .expect("Failed to bind gradient texture")
            .lock()
            .destroy(context.renderer);
        mesh_rendering_ref
            .lock()
            .upload_uniform(4, self.shader_options)
            .expect("Failed to upload flow settings");
        self.mesh_rendering_ref = Some(mesh_rendering_ref.clone());

        let mut tranform = Transform::default();
        tranform
            .translate(&glm::vec3(0.0, 0.0, -15.0))
            .rotate(f32::to_radians(-90.0), Axis::X)
            .scale(&glm::vec3(4.0, 4.0, 4.0));

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

    fn on_update_imgui(&mut self, ui: &mut imgui::Ui, _context: &mut StateContext) {
        if let Some(window) = imgui::Window::new("shader uniforms").begin(ui) {
            imgui::Slider::new("speed", 0.0_f32, 1.0_f32)
                .build(ui, &mut self.shader_options.flow_speed);
            imgui::Slider::new("intensity", 0.0_f32, 1.0_f32)
                .build(ui, &mut self.shader_options.flow_intensity);

            if ui.button("apply") {
                self.mesh_rendering_ref
                    .as_ref()
                    .unwrap()
                    .lock()
                    .upload_uniform(4, self.shader_options)
                    .expect("Failed to upload flow settings");
            }

            window.end();
        }
    }

    fn on_drop(&mut self, context: &mut StateContext) {
        self.gradient_ref
            .as_ref()
            .unwrap()
            .lock()
            .destroy(context.renderer);
        self.flowmap_ref
            .as_ref()
            .unwrap()
            .lock()
            .destroy(context.renderer);
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
