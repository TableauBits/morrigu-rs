use morrigu::{
    application::{event, ApplicationBuilder, ApplicationState, Window},
    components::mesh_renderer::MeshRenderer,
    material::MaterialBuilder,
    renderer::Renderer,
    shader::Shader,
    texture::Texture,
};

use std::path::Path;

type Vertex = morrigu::sample_vertex::TexturedVertex;

struct MachaState {
    frame_count: i32,
}

impl ApplicationState for MachaState {
    fn on_attach(&mut self, renderer: &mut Renderer, _window: &Window) {
        let test_shader = Shader::from_path(
            Path::new("assets/gen/shaders/test/test.vert"),
            Path::new("assets/gen/shaders/test/test.frag"),
            &renderer.device,
        )
        .expect("Failed to create shader");

        log::trace!("{:#?}", test_shader.vertex_bindings);
        log::trace!("{:#?}", test_shader.fragment_bindings);

        let test_material = MaterialBuilder::new()
            .build::<Vertex>(&test_shader, renderer)
            .expect("Failed to create material");

        let test_mesh =
            Vertex::load_model_from_path(Path::new("assets/meshes/monkey.obj"), renderer)
                .expect("Failed to load mesh");

        let test_mesh_renderer = MeshRenderer::new(&test_mesh, &test_material, renderer)
            .expect("Failed to rceate mesh renderer");

        let test_texture = Texture::from_path(Path::new("assets/img/rust.png"), renderer)
            .expect("Failed to create texture");
        log::trace!("texture path: {}", test_texture.path.as_ref().unwrap());

        test_texture.destroy(renderer);
        test_mesh_renderer.destroy(renderer);
        test_mesh.destroy(renderer);
        test_material.destroy(renderer);
        test_shader.destroy(&renderer.device);
    }

    fn on_update(&mut self, dt: std::time::Duration, _renderer: &mut Renderer, window: &Window) {
        self.frame_count += 1;
        if dt.as_millis() > 15 {
            let string = format!("frame {} handled in {}ms", self.frame_count, dt.as_millis());
            log::warn!("{}", string);
            window.set_title(&string);
        }
    }

    fn on_event(&mut self, event: event::Event<()>, _renderer: &mut Renderer, _window: &Window) {
        match event {
            event::Event::DeviceEvent {
                event: event::DeviceEvent::Button { button, state },
                ..
            } => {
                log::debug!("Mouse button detected: {:?}, {:?}", button, state);
            }
            _ => (),
        }
    }

    fn on_drop(&mut self, _renderer: &mut Renderer, _window: &Window) {}
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

    let mut state = MachaState { frame_count: 0 };
    ApplicationBuilder::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .build_and_run(&mut state);
}
