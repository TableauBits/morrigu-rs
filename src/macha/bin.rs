use morrigu::application::ApplicationBuilder;

fn main() {
    let app = ApplicationBuilder::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .build();
}
