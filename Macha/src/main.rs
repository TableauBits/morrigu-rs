use morrigu::application::{
    event, renderer::Renderer, ApplicationBuilder, ApplicationState, Window,
};

struct MachaState {
    frame_count: i32,
}

impl ApplicationState for MachaState {
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

    let mut state = MachaState { frame_count: 0 };
    ApplicationBuilder::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .build_and_run(&mut state);
}
