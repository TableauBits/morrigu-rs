pub use winit::{
    event::{self, Event},
    window::Window,
};

use crate::{
    components::camera::{Camera, PerspectiveData, Projection},
    ecs_manager::ECSManager,
    renderer::{Renderer, RendererBuilder},
    utils::ThreadSafeRef,
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

pub struct StateContext<'a> {
    pub renderer: &'a mut Renderer,
    pub ecs_manager: &'a mut ECSManager,
    pub window: &'a Window,
}

pub trait ApplicationState {
    fn on_attach(&mut self, _context: &mut StateContext) {}
    fn on_update(&mut self, _dt: Duration, _context: &mut StateContext) {}
    #[cfg(feature = "egui")]
    fn on_update_egui(&mut self, _egui_context: &egui::Context, _context: &mut StateContext) {}
    fn on_event(&mut self, _event: Event<()>, _context: &mut StateContext) {}
    fn on_drop(&mut self, _context: &mut StateContext) {}
}

pub trait BuildableApplicationState<UserData>: ApplicationState {
    fn build(context: &mut StateContext, data: UserData) -> Self;
}

struct ApplicationContext {
    #[cfg(feature = "egui")]
    pub egui: crate::egui::EguiIntegration,

    pub ecs_manager: ECSManager,
    pub renderer_ref: ThreadSafeRef<Renderer>,
    pub window: Window,
    pub event_loop: EventLoop<()>,
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

    fn setup_context(&self) -> ApplicationContext {
        let event_loop = EventLoop::new();

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
        let ecs_manager = ECSManager::new(
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

        #[cfg(feature = "egui")]
        {
            let mut renderer = renderer_ref.lock();
            let egui = crate::egui::EguiIntegration::new(&window, &mut renderer)
                .expect("Failed to create Egui intergration");
            drop(renderer);

            ApplicationContext {
                egui,
                ecs_manager,
                renderer_ref,
                window,
                event_loop,
            }
        }

        #[cfg(not(feature = "egui"))]
        {
            ApplicationContext {
                ecs_manager,
                renderer_ref,
                window,
                event_loop,
            }
        }
    }

    fn main_loop(&self, context: &mut ApplicationContext, state: &mut dyn ApplicationState) {
        #[cfg(feature = "egui")]
        let egui = &mut context.egui;

        let ecs_manager = &mut context.ecs_manager;
        let renderer_ref = &context.renderer_ref;
        let window = &context.window;
        let event_loop = &mut context.event_loop;

        let mut prev_time = Instant::now();

        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            #[cfg(feature = "egui")]
            if egui.handle_event(&event) {
                return;
            }

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
                    prev_time = Instant::now();

                    let mut renderer = renderer_ref.lock();
                    if renderer.begin_frame() {
                        #[cfg(feature = "egui")]
                        egui.painter.cleanup_previous_frame(&mut renderer);

                        state.on_update(
                            delta,
                            &mut StateContext {
                                renderer: &mut renderer,
                                ecs_manager,
                                window,
                            },
                        );
                        drop(renderer);

                        ecs_manager.run_schedule();

                        #[cfg(feature = "egui")]
                        {
                            let mut renderer = renderer_ref.lock();
                            egui.run(&context.window, |egui_context| {
                                state.on_update_egui(
                                    egui_context,
                                    &mut StateContext {
                                        renderer: &mut renderer,
                                        ecs_manager,
                                        window,
                                    },
                                );
                            });
                            egui.paint(&mut renderer)
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
                    ecs_manager,
                    window,
                },
            );
        });
    }

    fn exit(&self, context: &mut ApplicationContext, state: &mut dyn ApplicationState) {
        let mut renderer = context.renderer_ref.lock();
        unsafe {
            renderer
                .device
                .device_wait_idle()
                .expect("Failed to wait for device");
        }
        state.on_drop(&mut StateContext {
            renderer: &mut renderer,
            ecs_manager: &mut context.ecs_manager,
            window: &context.window,
        });

        context.egui.painter.destroy(&mut renderer);
    }

    pub fn build_and_run_inplace<StateType, UserData>(self, data: UserData)
    where
        StateType: BuildableApplicationState<UserData>,
    {
        let mut context = self.setup_context();

        let mut renderer = context.renderer_ref.lock();
        let mut state_context = StateContext {
            renderer: &mut renderer,
            ecs_manager: &mut context.ecs_manager,
            window: &context.window,
        };
        let mut state = StateType::build(&mut state_context, data);
        state.on_attach(&mut state_context);
        drop(renderer);

        self.main_loop(&mut context, &mut state);

        self.exit(&mut context, &mut state);
    }

    pub fn build_and_run(self, state: &mut impl ApplicationState) {
        let mut context = self.setup_context();

        let mut renderer = context.renderer_ref.lock();
        state.on_attach(&mut StateContext {
            renderer: &mut renderer,
            ecs_manager: &mut context.ecs_manager,
            window: &context.window,
        });
        drop(renderer);

        self.main_loop(&mut context, state);

        self.exit(&mut context, state);
    }
}

impl<'a> Default for ApplicationBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
