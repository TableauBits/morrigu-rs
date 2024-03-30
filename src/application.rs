pub use winit::{
    event::{self, Event},
    window::Window,
};

use crate::{
    components::camera::{Camera, PerspectiveData, Projection},
    ecs_manager::ECSManager,
    math_types::Vec2,
    renderer::{Renderer, RendererBuilder},
    utils::ThreadSafeRef,
};

use ash::vk;
use winit::{
    dpi::PhysicalSize,
    event_loop::{ControlFlow, EventLoop},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use std::time::{Duration, Instant};

pub struct StateContext<'a> {
    #[cfg(feature = "egui")]
    pub egui: &'a mut crate::egui_integration::EguiIntegration,

    pub renderer: &'a mut Renderer,
    pub ecs_manager: &'a mut ECSManager,
    pub window: &'a Window,
    pub window_input_state: &'a WinitInputHelper,
}

#[cfg(feature = "egui")]
pub struct EguiUpdateContext<'a> {
    pub egui_context: &'a egui::Context,

    pub renderer: &'a mut Renderer,
    pub ecs_manager: &'a mut ECSManager,
    pub window: &'a Window,
    pub window_input_state: &'a WinitInputHelper,
}

pub trait ApplicationState {
    fn on_attach(&mut self, _context: &mut StateContext) {}
    fn on_update(&mut self, _dt: Duration, _context: &mut StateContext) {}
    fn after_systems(&mut self, _dt: Duration, _context: &mut StateContext) {}
    #[cfg(feature = "egui")]
    fn on_update_egui(&mut self, _dt: Duration, _context: &mut EguiUpdateContext) {}
    #[cfg(feature = "egui")]
    fn after_ui_systems(&mut self, _dt: Duration, _context: &mut EguiUpdateContext) {}
    fn on_event(&mut self, _event: Event<()>, _context: &mut StateContext) {}
    fn on_drop(&mut self, _context: &mut StateContext) {}
}

pub trait BuildableApplicationState<UserData>: ApplicationState {
    fn build(context: &mut StateContext, data: UserData) -> Self;
}

pub struct Application<'state> {
    #[cfg(feature = "egui")]
    egui: crate::egui_integration::EguiIntegration,

    ecs_manager: ECSManager,
    renderer_ref: ThreadSafeRef<Renderer>,
    window: Window,
    event_loop: EventLoop<()>,
    window_input_state: WinitInputHelper,

    state: Box<dyn ApplicationState + 'state>,
}

impl<'state> Application<'state> {
    fn main_loop(&mut self) {
        let mut prev_time = Instant::now();

        self.event_loop.set_control_flow(ControlFlow::Poll);
        self.event_loop
            .run_on_demand(|event, target| {
                let events_cleared = self.window_input_state.update(&event);

                #[cfg(feature = "egui")]
                if self.egui.handle_event(&self.window, &event) {
                    return;
                }

                if events_cleared {
                    if self.window_input_state.close_requested()
                        || self.window_input_state.destroyed()
                    {
                        target.exit();
                    }

                    if let Some(PhysicalSize { width, height }) =
                        self.window_input_state.window_resized()
                    {
                        self.renderer_ref.lock().on_resize(width, height);
                        self.ecs_manager.on_resize(width, height);
                    }

                    let delta = prev_time.elapsed();
                    prev_time = Instant::now();

                    let mut renderer = self.renderer_ref.lock();
                    if renderer.begin_frame() {
                        profiling::scope!("main loop");

                        #[cfg(feature = "egui")]
                        self.egui.painter.cleanup_previous_frame(&mut renderer);

                        let mut state_context = StateContext {
                            #[cfg(feature = "egui")]
                            egui: &mut self.egui,
                            renderer: &mut renderer,
                            ecs_manager: &mut self.ecs_manager,
                            window: &self.window,
                            window_input_state: &self.window_input_state,
                        };
                        {
                            profiling::scope!("on_update");
                            self.state.on_update(delta, &mut state_context);
                        }
                        drop(renderer);

                        {
                            profiling::scope!("ECS schedule");
                            self.ecs_manager.run_schedule();
                            let mut renderer = self.renderer_ref.lock();
                            let mut state_context = StateContext {
                                #[cfg(feature = "egui")]
                                egui: &mut self.egui,
                                renderer: &mut renderer,
                                ecs_manager: &mut self.ecs_manager,
                                window: &self.window,
                                window_input_state: &self.window_input_state,
                            };
                            self.state.after_systems(delta, &mut state_context);
                            drop(renderer);
                        }

                        #[cfg(feature = "egui")]
                        {
                            profiling::scope!("egui update");
                            let mut renderer = self.renderer_ref.lock();
                            self.egui.run(&self.window, |egui_context| {
                                let mut egui_update_context = EguiUpdateContext {
                                    egui_context,
                                    renderer: &mut renderer,
                                    ecs_manager: &mut self.ecs_manager,
                                    window: &self.window,
                                    window_input_state: &self.window_input_state,
                                };
                                self.state.on_update_egui(delta, &mut egui_update_context);
                                egui_update_context
                                    .ecs_manager
                                    .run_ui_schedule(egui_update_context.egui_context);
                                self.state.after_ui_systems(delta, &mut egui_update_context);
                            });

                            self.egui.paint(&mut renderer)
                        }

                        let mut renderer = self.renderer_ref.lock();
                        renderer.end_frame();
                        profiling::finish_frame!();
                    }
                }

                let mut renderer = self.renderer_ref.lock();
                let mut state_context = StateContext {
                    #[cfg(feature = "egui")]
                    egui: &mut self.egui,
                    renderer: &mut renderer,
                    ecs_manager: &mut self.ecs_manager,
                    window: &self.window,
                    window_input_state: &self.window_input_state,
                };
                self.state.on_event(event, &mut state_context);
            })
            .expect("Error encountered during main loop");
    }

    fn exit(&mut self) {
        let mut renderer = self.renderer_ref.lock();
        unsafe {
            renderer
                .device
                .device_wait_idle()
                .expect("Failed to wait for device");
        }
        let mut state_context = StateContext {
            #[cfg(feature = "egui")]
            egui: &mut self.egui,
            renderer: &mut renderer,
            ecs_manager: &mut self.ecs_manager,
            window: &self.window,
            window_input_state: &self.window_input_state,
        };
        self.state.on_drop(&mut state_context);

        #[cfg(feature = "egui")]
        self.egui.painter.destroy(&mut renderer);
    }

    pub fn run(&mut self) {
        {
            let instant = Instant::now();

            let mut renderer = self.renderer_ref.lock();
            let mut state_context = StateContext {
                #[cfg(feature = "egui")]
                egui: &mut self.egui,
                renderer: &mut renderer,
                ecs_manager: &mut self.ecs_manager,
                window: &self.window,
                window_input_state: &self.window_input_state,
            };
            self.state.on_attach(&mut state_context);
            let engine_init_time = instant.elapsed();
            log::debug!(
                "Custom state attach time: {}ms",
                engine_init_time.as_millis()
            );
        }

        self.main_loop();

        let instant = Instant::now();
        self.exit();
        let engine_shut_down_time = instant.elapsed();
        log::debug!(
            "Custom state shut down time: {}ms",
            engine_shut_down_time.as_millis()
        );
        log::debug!("Engine shut down");
    }
}

pub struct ApplicationBuilder<'a> {
    width: u32,
    height: u32,
    window_name: &'a str,
    application_name: &'a str,
    version: (u32, u32, u32),
    preferred_present_mode: vk::PresentModeKHR,
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

    pub fn build_with_state<'state, StateType, UserData>(
        self,
        data: UserData,
    ) -> Application<'state>
    where
        StateType: BuildableApplicationState<UserData> + 'state,
    {
        let event_loop = EventLoop::new().expect("Failed to create program event loop");
        let window = WindowBuilder::new()
            .with_title(self.window_name)
            .with_inner_size(PhysicalSize::new(self.width, self.height))
            .with_resizable(true)
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
                &Vec2::new(self.width as f32, self.height as f32),
            ),
        );

        let winit_state = WinitInputHelper::new();

        #[cfg(feature = "egui")]
        {
            let mut renderer = renderer_ref.lock();
            let mut egui = crate::egui_integration::EguiIntegration::new(&window, &mut renderer)
                .expect("Failed to create Egui integration");

            let state = StateType::build(
                &mut StateContext {
                    egui: &mut egui,
                    renderer: &mut renderer,
                    ecs_manager: &mut ecs_manager,
                    window: &window,
                    window_input_state: &winit_state,
                },
                data,
            );

            drop(renderer);

            Application {
                egui,
                ecs_manager,
                renderer_ref,
                window,
                event_loop,
                window_input_state: winit_state,
                state: Box::new(state),
            }
        }

        #[cfg(not(feature = "egui"))]
        {
            let mut renderer = renderer_ref.lock();
            let state = StateType::build(
                &mut StateContext {
                    renderer: &mut renderer,
                    ecs_manager: &mut ecs_manager,
                    window: &window,
                    window_input_state: &winit_state,
                },
                data,
            );

            drop(renderer);

            Application {
                ecs_manager,
                renderer_ref,
                window,
                event_loop,
                window_input_state: winit_state,
                state: Box::new(state),
            }
        }
    }
}

impl<'a> Default for ApplicationBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}
