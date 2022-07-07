use bevy_ecs::schedule::SystemStage;
use morrigu::{
    application::{ApplicationBuilder, ApplicationState, BuildableApplicationState, StateContext},
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

struct MachaState {
    shader_ref: ThreadSafeRef<Shader>,
    material_ref: ThreadSafeRef<Material>,
    mesh_ref: ThreadSafeRef<Mesh>,
    mesh_rendering_ref: ThreadSafeRef<MeshRendering>,
    texture_ref: ThreadSafeRef<Texture>,
    flowmap_ref: ThreadSafeRef<Texture>,
    gradient_ref: ThreadSafeRef<Texture>,

    shader_options: glm::Vec2,
}

impl BuildableApplicationState<()> for MachaState {
    fn build(context: &mut StateContext, _: ()) -> Self {
        let flow_speed = 0.2_f32;
        let flow_intensity = 0.4_f32;
        let shader_options = glm::vec2(flow_speed, flow_intensity);

        let camera = Camera::builder().build(
            morrigu::components::camera::Projection::Perspective(PerspectiveData {
                horizontal_fov: f32::to_radians(90.0),
                near_plane: 0.001,
                far_plane: 1000.0,
            }),
            16.0 / 9.0,
        );
        context.ecs_manager.world.insert_resource(camera);

        let shader_ref = Shader::from_spirv_u8(
            include_bytes!("../assets/gen/shaders/test/test.vert"),
            include_bytes!("../assets/gen/shaders/test/test.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create shader");
        let material_ref = Material::builder()
            .build(&shader_ref, context.renderer)
            .expect("Failed to create material");

        let mesh_ref = Vertex::load_model_from_path_ply(
            Path::new("assets/meshes/sphere.ply"),
            context.renderer,
        )
        .expect("Failed to create mesh");

        let texture_ref = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .build_from_path(
                Path::new("assets/textures/jupiter_base.png"),
                context.renderer,
            )
            .expect("Failed to load texture");
        let flowmap_ref = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .build_from_path(
                Path::new("assets/textures/jupiter_flowmap.png"),
                context.renderer,
            )
            .expect("Failed to load flowmap texture");
        let gradient_ref = Texture::builder()
            .build_from_path(
                Path::new("assets/textures/jupiter_gradient.png"),
                context.renderer,
            )
            .expect("Failed to load gradient texture");

        let mesh_rendering_ref = MeshRendering::new(&mesh_ref, &material_ref, context.renderer)
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
            .upload_uniform(4, shader_options)
            .expect("Failed to upload flow settings");

        let mut tranform = Transform::default();
        tranform
            .translate(&glm::vec3(0.0, 0.0, -15.0))
            .rotate(f32::to_radians(-90.0), Axis::X)
            .scale(&glm::vec3(4.0, 4.0, 4.0));

        context
            .ecs_manager
            .world
            .spawn()
            .insert(tranform)
            .insert(mesh_rendering_ref.clone());

        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_stage(
                "render meshes",
                SystemStage::parallel().with_system(mesh_renderer::render_meshes::<Vertex>),
            );
        });

        MachaState {
            shader_ref,
            material_ref,
            mesh_ref,
            mesh_rendering_ref,
            texture_ref,
            flowmap_ref,
            gradient_ref,
            shader_options,
        }
    }
}

impl ApplicationState for MachaState {
    fn on_update_egui(
        &mut self,
        dt: std::time::Duration,
        egui_context: &egui::Context,
        _context: &mut StateContext,
    ) {
        egui::Window::new("Debug info").show(egui_context, |ui| {
            let color = match dt.as_millis() {
                0..=25 => [51, 204, 51],
                26..=50 => [255, 153, 0],
                _ => [204, 51, 51],
            };
            ui.colored_label(
                egui::Color32::from_rgb(color[0], color[1], color[2]),
                format!("FPS: {} ({}ms)", 1.0 / dt.as_secs_f32(), dt.as_millis()),
            );
        });
        egui::Window::new("Shader uniforms").show(egui_context, |ui| {
            ui.add(egui::Slider::new(&mut self.shader_options[0], 0.0..=1.0).text("flow speed"));
            ui.add(
                egui::Slider::new(&mut self.shader_options[1], 0.0..=1.0).text("flow intensity"),
            );

            if ui.button("Apply changes").clicked() {
                self.mesh_rendering_ref
                    .lock()
                    .upload_uniform(4, self.shader_options)
                    .expect("Failed to upload flow settings");
            }

            ui.allocate_space(ui.available_size());
        });
    }

    fn on_drop(&mut self, context: &mut StateContext) {
        self.gradient_ref.lock().destroy(context.renderer);
        self.flowmap_ref.lock().destroy(context.renderer);
        self.texture_ref.lock().destroy(context.renderer);
        self.mesh_rendering_ref.lock().destroy(context.renderer);
        self.mesh_ref.lock().destroy(context.renderer);
        self.material_ref.lock().destroy(context.renderer);
        self.shader_ref.lock().destroy(&context.renderer.device);
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

    ApplicationBuilder::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .build_and_run_inplace::<MachaState, ()>(());
}
