pub use winit::{
    event::{self, Event},
    window::Window,
};

use crate::{
    components::camera::{Camera, PerspectiveData, Projection},
    ecs_manager::ECSManager,
    renderer::{Renderer, RendererBuilder},
};

use ash::vk;
use winit::{
    dpi::PhysicalSize,
    event::Event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};

use std::time::{Duration, Instant};

struct ImGui {
    pub context: imgui::Context,
    pub platform: imgui_winit_support::WinitPlatform,
    pub renderer: imgui_rs_vulkan_renderer::Renderer,
}

impl ImGui {
    fn new(renderer: &mut Renderer, window: &Window) -> Self {
        let mut context = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut context);

        let highdpi_factor = platform.hidpi_factor() as f32;
        let font_size = 13.0 * highdpi_factor;
        context
            .fonts()
            .add_font(&[imgui::FontSource::DefaultFontData {
                config: Some(imgui::FontConfig {
                    size_pixels: font_size,
                    ..Default::default()
                }),
            }]);
        context.io_mut().font_global_scale = 1.0 / highdpi_factor;

        platform.attach_window(
            context.io_mut(),
            window,
            imgui_winit_support::HiDpiMode::Rounded,
        );

        let renderer = renderer
            .create_imgui_renderer(&mut context)
            .expect("Failed to build imgui renderer");

        ImGui {
            context,
            platform,
            renderer,
        }
    }
}

pub struct StateContext<'a> {
    pub renderer: &'a mut Renderer,
    pub ecs_manager: &'a mut ECSManager,
    pub window: &'a Window,
}

pub trait ApplicationState {
    fn on_attach(&mut self, _context: &mut StateContext) {}
    fn on_update(&mut self, _dt: Duration, _context: &mut StateContext) {}
    fn on_update_imgui(&mut self, _ui: &mut imgui::Ui, _context: &mut StateContext) {}
    fn on_event(&mut self, _event: Event<()>, _context: &mut StateContext) {}
    fn on_drop(&mut self, _context: &mut StateContext) {}
}

pub struct ApplicationBuilder<'a> {
    width: u32,
    height: u32,
    window_name: &'a str,
    application_name: &'a str,
    version: (u32, u32, u32),
    preferred_present_mode: vk::PresentModeKHR,
    // input_attachments: Vec<(vk::AttachmentDescription, vk::AttachmentReference)>,
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
            // input_attachments: vec![],
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

        let renderer_ref = RendererBuilder::new(&window)
            .with_dimensions(self.width, self.height)
            .with_preferred_present_mode(self.preferred_present_mode)
            .with_name(self.application_name)
            .with_version(self.version.0, self.version.1, self.version.2)
            .build();
        let mut ecs_manager = ECSManager::new(
            &renderer_ref,
            Camera::builder().build(
                Projection::Perspective(PerspectiveData {
                    horizontal_fov: f32::to_radians(90.0),
                    near_plane: 0.001,
                    far_plane: 1000.0,
                }),
                self.width as f32 / self.height as f32,
            ),
        );

        let mut renderer = renderer_ref.lock();
        let mut imgui = ImGui::new(&mut renderer, &window);

        state.on_attach(&mut StateContext {
            renderer: &mut renderer,
            ecs_manager: &mut ecs_manager,
            window: &window,
        });
        drop(renderer);

        let mut prev_time = Instant::now();

        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            imgui
                .platform
                .handle_event(imgui.context.io_mut(), &window, &event);

            match event {
                WindowEvent {
                    event: event::WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                WindowEvent {
                    event: event::WindowEvent::Resized(PhysicalSize { width, height, .. }),
                    ..
                } => {
                    renderer_ref.lock().on_resize(width, height);
                    ecs_manager.on_resize(width, height);
                }
                Event::MainEventsCleared => {
                    let delta = prev_time.elapsed();
                    imgui.context.io_mut().update_delta_time(delta);
                    prev_time = Instant::now();

                    let mut renderer = renderer_ref.lock();
                    if renderer.begin_frame() {
                        state.on_update(
                            delta,
                            &mut StateContext {
                                renderer: &mut renderer,
                                ecs_manager: &mut ecs_manager,
                                window: &window,
                            },
                        );
                        drop(renderer);

                        ecs_manager.run_schedule();

                        #[cfg(debug_assertions)]
                        {
                            if let Err(error) = imgui
                                .platform
                                .prepare_frame(imgui.context.io_mut(), &window)
                            {
                                log::error!("ImGui error while preparing frame: {}", error);
                            }

                            let mut ui = imgui.context.frame();
                            let mut renderer = renderer_ref.lock();
                            state.on_update_imgui(
                                &mut ui,
                                &mut StateContext {
                                    renderer: &mut renderer,
                                    ecs_manager: &mut ecs_manager,
                                    window: &window,
                                },
                            );
                            imgui.platform.prepare_render(&ui, &window);
                            let draw_data = ui.render();

                            imgui
                                .renderer
                                .cmd_draw(renderer.primary_command_buffer, draw_data)
                                .expect("Failed to render UI");
                        }

                        let renderer = renderer_ref.lock();
                        renderer.end_frame();
                    }
                }
                _ => (),
            }

            let mut renderer = renderer_ref.lock();
            state.on_event(
                event,
                &mut StateContext {
                    renderer: &mut renderer,
                    ecs_manager: &mut ecs_manager,
                    window: &window,
                },
            );
        });

        let mut renderer = renderer_ref.lock();
        unsafe {
            renderer
                .device
                .device_wait_idle()
                .expect("Failed to wait for device");
        }
        state.on_drop(&mut StateContext {
            renderer: &mut renderer,
            ecs_manager: &mut ecs_manager,
            window: &window,
        });
    }
}

impl<'a> Default for ApplicationBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
