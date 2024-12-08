mod utils;

mod compute_shader_test;
mod editor;
mod gltf_loader;
mod pbr_test;

#[cfg(feature = "ray_tracing")]
mod rt_test;

use morrigu::application::{Application, ApplicationConfiguration};

use clap::Parser;
use utils::startup_state::{StartupState, SwitchableStates};

fn init_logging() {
    #[cfg(debug_assertions)]
    let log_level = ("debug", flexi_logger::Duplicate::Debug);
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

#[derive(Parser)]
struct Args {
    #[arg(value_enum)]
    startup_state: Option<SwitchableStates>,
}

fn main() {
    let args = Args::parse();

    init_logging();

    let desired_state = args.startup_state.unwrap_or(SwitchableStates::Editor);

    let app_config = ApplicationConfiguration::new()
        .with_window_name("Macha".to_owned())
        .with_dimensions(1280, 720)
        .with_application_name("Macha".to_owned())
        .with_application_version(0, 1, 0);

    Application::<StartupState, SwitchableStates>::run(app_config, desired_state);
}
