use morrigu::application::{renderer::Renderer, ApplicationBuilder, ApplicationState};

struct MachaApplication {
    frame_count: i32,
}

impl ApplicationState for MachaApplication {
    fn on_update(&mut self, dt: std::time::Duration, _renderer: &mut Renderer) {
        self.frame_count += 1;
        if dt.as_millis() > 15 {
            println!("frame {} handled in {}ms", self.frame_count, dt.as_millis());
        }
    }
}

fn init_logging() {
    #[allow(unused_assignments)]
    #[allow(unused_mut)]
    let mut env = env_logger::Env::default().default_filter_or("info");
    #[cfg(debug_assertions)]
    {
        env = env_logger::Env::default().default_filter_or("debug");
    }
    env_logger::Builder::from_env(env).init();
}

fn main() {
    init_logging();

    let mut state = MachaApplication { frame_count: 0 };
    ApplicationBuilder::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .build_and_run(&mut state);
}
