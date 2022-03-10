mod allocated_types;
pub mod renderer;

use std::time::{Duration, Instant};

use ash::vk;
use winit::{
    dpi::PhysicalSize,
    event::{self, Event::WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use renderer::{Renderer, RendererBuilder};

pub trait ApplicationState {
    fn on_update(&mut self, dt: Duration, renderer: &mut Renderer);
}

pub struct ApplicationBuilder<'a> {
    width: u32,
    height: u32,
    window_name: &'a str,
    application_name: &'a str,
    version: (u32, u32, u32),
    preferred_present_mode: vk::PresentModeKHR,
    input_attachments: Vec<(vk::AttachmentDescription, vk::AttachmentReference)>,
}

impl<'a> ApplicationBuilder<'a> {
    pub fn new() -> Self {
        ApplicationBuilder {
            width: 1280,
            height: 720,
            window_name: "Morrigu application",
            application_name: "Morrigu application",
            version: (0, 0, 0),
            preferred_present_mode: vk::PresentModeKHR::MAILBOX,
            input_attachments: vec![],
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

    pub fn with_preferred_present_mode(mut self, present_mode: vk::PresentModeKHR) -> Self {
        self.preferred_present_mode = present_mode;
        self
    }

    // NOT SUPPORTED YET
    /*
    pub fn with_input_attachments(
        mut self,
        input_attachments: Vec<(vk::AttachmentDescription, vk::AttachmentReference)>,
    ) -> Self {
        self.input_attachments = input_attachments;
        self
    }
    */

    pub fn build_and_run(self, state: &mut impl ApplicationState) {
        let mut event_loop = EventLoop::new();

        let window = WindowBuilder::new()
            .with_inner_size(PhysicalSize::new(self.width, self.height))
            .with_title(self.window_name)
            .build(&event_loop)
            .expect("Failed to create window");

        let mut renderer = RendererBuilder::new(&window)
            .with_dimensions(self.width, self.height)
            .with_preferred_present_mode(self.preferred_present_mode)
            .with_name(self.application_name)
            .with_version(self.version.0, self.version.1, self.version.2)
            .build();

        let mut prev_time = Instant::now();

        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            let delta = prev_time.elapsed();
            prev_time = Instant::now();

            if renderer.begin_frame() {
                state.on_update(delta, &mut renderer);
                renderer.end_frame();
            }

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

impl<'a> Default for ApplicationBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
