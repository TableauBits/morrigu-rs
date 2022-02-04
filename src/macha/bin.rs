use morrigu::application::{ApplicationConfig, ApplicationState};

struct MachaApplication {
    frame_count: i32,
}

impl ApplicationState for MachaApplication {
    fn on_update(&mut self, dt: std::time::Duration) {
        println!("frame {} -> {}ms", self.frame_count, dt.as_millis());
        self.frame_count += 1;
    }
}

fn main() {
    let mut state = MachaApplication { frame_count: 0 };
    ApplicationConfig::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .run(&mut state);
}
