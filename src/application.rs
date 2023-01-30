pub use winit::{
    event::{self, Event},
    window::Window,
};

use crate::{
    components::camera::{Camera, PerspectiveData, Projection},
    ecs_manager::ECSManager,
    renderer::{Renderer, RendererBuilder},
    utils::ThreadSafeRef,
    vector_type::Vec2,
};

use ash::vk;
use winit::{
    dpi::PhysicalSize,
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use std::time::{Duration, Instant};

pub struct StateContext<'a> {
    #[cfg(feature = "egui")]
    pub egui: &'a mut crate::egui::EguiIntegration,

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

struct ApplicationContext {
    #[cfg(feature = "egui")]
    pub egui: crate::egui::EguiIntegration,

    pub ecs_manager: ECSManager,
    pub renderer_ref: ThreadSafeRef<Renderer>,
    pub window: Window,
    pub event_loop: EventLoop<()>,
    pub window_input_state: WinitInputHelper,
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
        let ecs_manager = ECSManager::new(
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
            let egui = crate::egui::EguiIntegration::new(&event_loop, &mut renderer)
                .expect("Failed to create Egui intergration");
            drop(renderer);

            ApplicationContext {
                egui,
                ecs_manager,
                renderer_ref,
                window,
                event_loop,
                window_input_state: winit_state,
            }
        }

        #[cfg(not(feature = "egui"))]
        {
            ApplicationContext {
                ecs_manager,
                renderer_ref,
                window,
                event_loop,
                window_input_state: winit_state,
            }
        }
    }

    fn main_loop(&self, context: &mut ApplicationContext, state: &mut dyn ApplicationState) {
        let mut prev_time = Instant::now();

        context.event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            let events_cleared = context.window_input_state.update(&event);

            #[cfg(feature = "egui")]
            if context.egui.handle_event(&event) {
                return;
            }

            if events_cleared {
                if context.window_input_state.quit() {
                    control_flow.set_exit();
                }

                if let Some(PhysicalSize { width, height }) =
                    context.window_input_state.window_resized()
                {
                    context.renderer_ref.lock().on_resize(width, height);
                    context.ecs_manager.on_resize(width, height);
                }

                let delta = prev_time.elapsed();
                prev_time = Instant::now();

                let mut renderer = context.renderer_ref.lock();
                if renderer.begin_frame() {
                    #[cfg(feature = "egui")]
                    context.egui.painter.cleanup_previous_frame(&mut renderer);

                    let mut state_context = StateContext {
                        #[cfg(feature = "egui")]
                        egui: &mut context.egui,
                        renderer: &mut renderer,
                        ecs_manager: &mut context.ecs_manager,
                        window: &context.window,
                        window_input_state: &context.window_input_state,
                    };
                    state.on_update(delta, &mut state_context);
                    drop(renderer);

                    context.ecs_manager.run_schedule();
                    let mut renderer = context.renderer_ref.lock();
                    let mut state_context = StateContext {
                        #[cfg(feature = "egui")]
                        egui: &mut context.egui,
                        renderer: &mut renderer,
                        ecs_manager: &mut context.ecs_manager,
                        window: &context.window,
                        window_input_state: &context.window_input_state,
                    };
                    state.after_systems(delta, &mut state_context);
                    drop(renderer);

                    #[cfg(feature = "egui")]
                    {
                        let mut renderer = context.renderer_ref.lock();
                        context.egui.run(&context.window, |egui_context| {
                            let mut egui_update_context = EguiUpdateContext {
                                egui_context,
                                renderer: &mut renderer,
                                ecs_manager: &mut context.ecs_manager,
                                window: &context.window,
                                window_input_state: &context.window_input_state,
                            };
                            state.on_update_egui(delta, &mut egui_update_context);
                            egui_update_context
                                .ecs_manager
                                .run_ui_schedule(egui_update_context.egui_context);
                            state.after_ui_systems(delta, &mut egui_update_context);
                        });

                        context.egui.paint(&mut renderer)
                    }

                    let mut renderer = context.renderer_ref.lock();
                    renderer.end_frame();
                }
            }

            let mut renderer = context.renderer_ref.lock();
            let mut state_context = StateContext {
                #[cfg(feature = "egui")]
                egui: &mut context.egui,
                renderer: &mut renderer,
                ecs_manager: &mut context.ecs_manager,
                window: &context.window,
                window_input_state: &context.window_input_state,
            };
            state.on_event(event, &mut state_context);
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
        let mut state_context = StateContext {
            #[cfg(feature = "egui")]
            egui: &mut context.egui,
            renderer: &mut renderer,
            ecs_manager: &mut context.ecs_manager,
            window: &context.window,
            window_input_state: &context.window_input_state,
        };
        state.on_drop(&mut state_context);

        #[cfg(feature = "egui")]
        context.egui.painter.destroy(&mut renderer);
    }

    pub fn build_and_run_inplace<StateType, UserData>(self, data: UserData)
    where
        StateType: BuildableApplicationState<UserData>,
    {
        let instant = std::time::Instant::now();
        let mut context = self.setup_context();
        let engine_init_time = instant.elapsed();
        log::debug!("Engine startup time: {}ms", engine_init_time.as_millis());

        let mut renderer = context.renderer_ref.lock();
        let mut state_context = StateContext {
            #[cfg(feature = "egui")]
            egui: &mut context.egui,
            renderer: &mut renderer,
            ecs_manager: &mut context.ecs_manager,
            window: &context.window,
            window_input_state: &context.window_input_state,
        };

        let instant = std::time::Instant::now();
        let mut state = StateType::build(&mut state_context, data);
        let engine_init_time = instant.elapsed();
        log::debug!(
            "Custom state creation time: {}ms",
            engine_init_time.as_millis()
        );

        let instant = std::time::Instant::now();
        state.on_attach(&mut state_context);
        let engine_init_time = instant.elapsed();
        log::debug!(
            "Custom state attach time: {}ms",
            engine_init_time.as_millis()
        );

        drop(renderer);

        self.main_loop(&mut context, &mut state);

        self.exit(&mut context, &mut state);
    }

    pub fn build_and_run(self, state: &mut impl ApplicationState) {
        let instant = std::time::Instant::now();
        let mut context = self.setup_context();
        let engine_init_time = instant.elapsed();
        log::debug!("Engine startup time: {}ms", engine_init_time.as_millis());

        let mut renderer = context.renderer_ref.lock();
        let mut state_context = StateContext {
            #[cfg(feature = "egui")]
            egui: &mut context.egui,
            renderer: &mut renderer,
            ecs_manager: &mut context.ecs_manager,
            window: &context.window,
            window_input_state: &context.window_input_state,
        };

        let instant = std::time::Instant::now();
        state.on_attach(&mut state_context);
        let engine_init_time = instant.elapsed();
        log::debug!(
            "Custom state attach time: {}ms",
            engine_init_time.as_millis()
        );

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
