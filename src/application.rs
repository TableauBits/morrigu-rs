pub use winit::{event, window::Window};
use winit_input_helper::WinitInputHelper;

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
    platform::{run_on_demand::EventLoopExtRunOnDemand, x11::WindowAttributesExtX11},
};

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

pub enum StateFlow<'state> {
    Continue,
    Exit,
    SwitchState(Box<dyn ApplicationState + 'state>),
}

pub trait ApplicationState {
    fn on_attach(&mut self, _context: &mut StateContext) {}
    fn on_drop(&mut self, _context: &mut StateContext) {}

    fn on_update(&mut self, _dt: Duration, _context: &mut StateContext) {}
    fn after_systems(&mut self, _dt: Duration, _context: &mut StateContext) {}
    #[cfg(feature = "egui")]
    fn on_update_egui(&mut self, _dt: Duration, _context: &mut EguiUpdateContext) {}
    #[cfg(feature = "egui")]
    fn after_ui_systems(&mut self, _dt: Duration, _context: &mut EguiUpdateContext) {}
    fn on_window_event(&mut self, _event: event::WindowEvent, _context: &mut StateContext) {}
    fn on_device_event(&mut self, _event: event::DeviceEvent, _context: &mut StateContext) {}

    fn flow<'flow>(&mut self, _context: &mut StateContext) -> StateFlow<'flow> {
        StateFlow::Continue
    }
}

pub trait BuildableApplicationState<UserData>: ApplicationState
where
    UserData: Clone,
{
    fn build(context: &mut StateContext, data: UserData) -> Self;
}

pub struct ApplicationConfiguration {
    width: u32,
    height: u32,
    window_name: String,
    application_name: String,
    version: (u32, u32, u32),
    preferred_present_mode: vk::PresentModeKHR,
}

impl ApplicationConfiguration {
    pub fn new() -> Self {
        ApplicationConfiguration {
            width: 1280,
            height: 720,
            window_name: "Morrigu application".to_owned(),
            application_name: "Morrigu application".to_owned(),
            version: (0, 0, 0),
            preferred_present_mode: vk::PresentModeKHR::MAILBOX,
        }
    }

    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn with_window_name(mut self, name: String) -> Self {
        self.window_name = name;
        self
    }

    pub fn with_application_name(mut self, name: String) -> Self {
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
}

impl Default for ApplicationConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

struct ApplicationData<'state> {
    #[cfg(feature = "egui")]
    egui: crate::egui_integration::EguiIntegration,

    ecs_manager: ECSManager,
    renderer_ref: ThreadSafeRef<Renderer>,
    window: Window,
    prev_time: std::time::Instant,
    window_input_state: WinitInputHelper,

    state: Box<dyn ApplicationState + 'state>,
}

impl ApplicationData<'_> {
    fn update(&mut self) {
        let delta = self.prev_time.elapsed();
        self.prev_time = Instant::now();

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

        self.window_input_state.end_step();
    }

    fn handle_window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: event::WindowEvent,
    ) {
        self.window_input_state.process_window_event(&event);

        if self.window_input_state.close_requested() || self.window_input_state.destroyed() {
            event_loop.exit();
        }

        #[cfg(feature = "egui")]
        if self.egui.handle_event(&self.window, &event) {
            return;
        }

        if let event::WindowEvent::Resized(PhysicalSize { width, height }) = event {
            self.renderer_ref.lock().on_resize(width, height);
            self.ecs_manager.on_resize(width, height);
        };

        let mut renderer = self.renderer_ref.lock();
        let mut state_context = StateContext {
            #[cfg(feature = "egui")]
            egui: &mut self.egui,
            renderer: &mut renderer,
            ecs_manager: &mut self.ecs_manager,
            window: &self.window,
            window_input_state: &self.window_input_state,
        };
        self.state.on_window_event(event, &mut state_context);

        match self.state.flow(&mut state_context) {
            StateFlow::Continue => (),
            StateFlow::Exit => event_loop.exit(),
            StateFlow::SwitchState(new_state) => {
                log::debug!("Switching states !");

                self.state.on_drop(&mut state_context);

                let res = (
                    self.window.inner_size().width,
                    self.window.inner_size().height,
                );

                let camera = Camera::builder().build(
                    Projection::Perspective(PerspectiveData {
                        horizontal_fov: f32::to_radians(90.0),
                        near_plane: 0.001,
                        far_plane: 1000.0,
                    }),
                    &Vec2::new(res.0 as f32, res.1 as f32),
                );
                *state_context.ecs_manager = ECSManager::new(&self.renderer_ref, camera);
                state_context.ecs_manager.on_resize(res.0, res.1);

                self.state = new_state;
                self.state.on_attach(&mut state_context);
            }
        }
    }

    fn handle_device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: event::DeviceEvent,
    ) {
        self.window_input_state.process_device_event(&event);

        if self.window_input_state.close_requested() || self.window_input_state.destroyed() {
            event_loop.exit();
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
        self.state.on_device_event(event, &mut state_context);

        match self.state.flow(&mut state_context) {
            StateFlow::Continue => (),
            StateFlow::Exit => event_loop.exit(),
            StateFlow::SwitchState(new_state) => {
                log::debug!("Switching states !");

                self.state.on_drop(&mut state_context);

                let res = (
                    self.window.inner_size().width,
                    self.window.inner_size().height,
                );

                let camera = Camera::builder().build(
                    Projection::Perspective(PerspectiveData {
                        horizontal_fov: f32::to_radians(90.0),
                        near_plane: 0.001,
                        far_plane: 1000.0,
                    }),
                    &Vec2::new(res.0 as f32, res.1 as f32),
                );
                *state_context.ecs_manager = ECSManager::new(&self.renderer_ref, camera);
                state_context.ecs_manager.on_resize(res.0, res.1);

                self.state = new_state;
                self.state.on_attach(&mut state_context);
            }
        }
    }

    fn on_exit(&mut self) {
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
}

enum ApplicationStatus<'state, UserData> {
    Uninit(UserData),
    Running(ApplicationData<'state>),
}

pub struct Application<'state, StartupStateType, UserData>
where
    StartupStateType: BuildableApplicationState<UserData> + 'state,
    UserData: Clone,
{
    app_config: ApplicationConfiguration,
    _initial_state: std::marker::PhantomData<StartupStateType>,

    status: ApplicationStatus<'state, UserData>,
}

impl<'state, StartupStateType, UserData> winit::application::ApplicationHandler
    for Application<'state, StartupStateType, UserData>
where
    StartupStateType: BuildableApplicationState<UserData> + 'state,
    UserData: Clone,
{
    fn new_events(&mut self, _: &winit::event_loop::ActiveEventLoop, cause: event::StartCause) {
        match cause {
            event::StartCause::Poll => match &mut self.status {
                ApplicationStatus::Uninit(_) => {
                    log::warn!("Attempting to update before initialization")
                }
                ApplicationStatus::Running(application_data) => {
                    application_data.window_input_state.step()
                }
            },
            event::StartCause::Init => {}
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        if let ApplicationStatus::Running(application_data) = &mut self.status {
            application_data.update();
        }
    }

    fn exiting(&mut self, _: &winit::event_loop::ActiveEventLoop) {
        match &mut self.status {
            ApplicationStatus::Uninit(_) => log::warn!("Attempting to exit before initialization"),
            ApplicationStatus::Running(application_data) => {
                let instant = Instant::now();

                application_data.on_exit();

                let engine_shut_down_time = instant.elapsed();
                log::debug!(
                    "Custom state shut down time: {}ms",
                    engine_shut_down_time.as_millis()
                );
                log::debug!("Engine shut down");
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        event: event::WindowEvent,
    ) {
        match &mut self.status {
            ApplicationStatus::Uninit(_) => {
                log::warn!("Window even received before initialization")
            }
            ApplicationStatus::Running(application_data) => {
                application_data.handle_window_event(event_loop, event)
            }
        }
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: event::DeviceId,
        event: event::DeviceEvent,
    ) {
        match &mut self.status {
            ApplicationStatus::Uninit(_) => {
                log::warn!("Device even received before initialization")
            }
            ApplicationStatus::Running(application_data) => {
                application_data.handle_device_event(event_loop, event)
            }
        }
    }

    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        match &self.status {
            ApplicationStatus::Uninit(data) => {
                let instant = Instant::now();

                let window_attributes = winit::window::Window::default_attributes()
                    .with_title(self.app_config.application_name.clone())
                    .with_base_size(PhysicalSize {
                        width: self.app_config.width,
                        height: self.app_config.height,
                    });
                let window = event_loop
                    .create_window(window_attributes)
                    .expect("Failed to create window");

                let window_input_state = WinitInputHelper::new();

                let renderer_ref = RendererBuilder::new(&window)
                    .with_dimensions(self.app_config.width, self.app_config.height)
                    .with_preferred_present_mode(self.app_config.preferred_present_mode)
                    .with_name(&self.app_config.application_name)
                    .with_version(
                        self.app_config.version.0,
                        self.app_config.version.1,
                        self.app_config.version.2,
                    )
                    .build();
                let mut ecs_manager = ECSManager::new(
                    &renderer_ref,
                    Camera::builder().build(
                        Projection::Perspective(PerspectiveData {
                            horizontal_fov: f32::to_radians(90.0),
                            near_plane: 0.001,
                            far_plane: 1000.0,
                        }),
                        &Vec2::new(self.app_config.width as f32, self.app_config.height as f32),
                    ),
                );

                let mut renderer = renderer_ref.lock();
                #[cfg(feature = "egui")]
                let mut egui =
                    crate::egui_integration::EguiIntegration::new(&window, &mut renderer)
                        .expect("Failed to create Egui integration");

                let mut state = StartupStateType::build(
                    &mut StateContext {
                        #[cfg(feature = "egui")]
                        egui: &mut egui,

                        renderer: &mut renderer,
                        ecs_manager: &mut ecs_manager,
                        window: &window,
                        window_input_state: &window_input_state,
                    },
                    data.clone(),
                );

                let mut state_context = StateContext {
                    #[cfg(feature = "egui")]
                    egui: &mut egui,

                    renderer: &mut renderer,
                    ecs_manager: &mut ecs_manager,
                    window: &window,
                    window_input_state: &window_input_state,
                };
                state.on_attach(&mut state_context);
                let engine_init_time = instant.elapsed();
                log::debug!(
                    "Custom state attach time: {}ms",
                    engine_init_time.as_millis()
                );

                let state = Box::new(state);

                drop(renderer);

                self.status = ApplicationStatus::Running(ApplicationData {
                    #[cfg(feature = "egui")]
                    egui,

                    ecs_manager,
                    renderer_ref,
                    window,
                    prev_time: Instant::now(),
                    window_input_state,

                    state,
                });
            }
            ApplicationStatus::Running(_) => {
                log::error!(
                    "Resume was called more than once, your platform is very likely not supported"
                );
                panic!();
            }
        }
    }
}

impl<'state, StartupStateType, UserData> Application<'state, StartupStateType, UserData>
where
    StartupStateType: BuildableApplicationState<UserData> + 'state,
    UserData: Clone,
{
    pub fn run(app_config: ApplicationConfiguration, data: UserData) {
        let mut event_loop = EventLoop::new().expect("Failed to create program event loop");
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = Self {
            app_config,
            status: ApplicationStatus::Uninit(data),

            _initial_state: Default::default(),
        };

        event_loop
            .run_app_on_demand(&mut app)
            .expect("Encountered error in main loop");
    }
}
