mod renderer;

use winit::{
    dpi::PhysicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use renderer::Renderer;

pub struct ApplicationBuilder<'a> {
    width: u32,
    height: u32,
    window_name: &'a str,
    application_name: &'a str,
    version: (u32, u32, u32),
}

pub struct Application {
    event_loop: EventLoop<()>,
    window: Window,
    renderer: Renderer,
}

impl<'a> ApplicationBuilder<'a> {
    pub fn new() -> ApplicationBuilder<'a> {
        ApplicationBuilder {
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

    pub fn build(self) -> Application {
        let event_loop = EventLoop::new();

        let window = WindowBuilder::new()
            .with_inner_size(PhysicalSize::new(self.width, self.height))
            .with_title(self.window_name)
            .build(&event_loop)
            .expect("Failed to create window!");

        let renderer = renderer::RendererBuilder::new(&window)
            .with_name(self.application_name)
            .with_version(self.version.0, self.version.1, self.version.2)
            .build();

        Application {
            event_loop,
            window,
            renderer,
        }
    }
}

impl Application {}
