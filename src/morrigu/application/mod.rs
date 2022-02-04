mod renderer;

use std::time::{Duration, Instant};

use winit::{
    dpi::PhysicalSize,
    event::{self, Event::WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::{Window, WindowBuilder},
};

use renderer::RendererBuilder;

pub trait ApplicationState {
    fn on_update(&mut self, dt: Duration);
}

pub struct ApplicationConfig<'a> {
    width: u32,
    height: u32,
    window_name: &'a str,
    application_name: &'a str,
    version: (u32, u32, u32),
}

impl<'a> ApplicationConfig<'a> {
    pub fn new() -> ApplicationConfig<'a> {
        ApplicationConfig {
            width: 1280,
            height: 720,
            window_name: "Morrigu sample application",
            application_name: "Morrigu sample application",
            version: (0, 0, 0),
        }
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_window_name(mut self, name: &'a str) -> Self {
        self.window_name = name;
        self
    }

    pub fn with_application_name(mut self, name: &'a str) -> Self {
        self.application_name = name;
        self
    }

    pub fn with_application_version(mut self, major: u32, minor: u32, patch: u32) -> Self {
        self.version = (major, minor, patch);
        self
    }

    pub fn run(self, state: &mut impl ApplicationState) {
        let mut event_loop = EventLoop::new();

        let window = WindowBuilder::new()
            .with_inner_size(PhysicalSize::new(self.width, self.height))
            .with_title(self.window_name)
            .build(&event_loop)
            .expect("Failed to create window!");

        let renderer = RendererBuilder::new(&window)
            .with_name(self.application_name)
            .with_version(self.version.0, self.version.1, self.version.2)
            .build();

        let mut prev_time = Instant::now();

        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            let delta = prev_time.elapsed();
            prev_time = Instant::now();

            state.on_update(delta);

            match event {
                WindowEvent {
                    event: event::WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                _ => (),
            }
        });
    }
}
