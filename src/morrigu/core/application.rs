use super::layer::{LayerRef, Transition};
use super::timestep::Timestep;
use crate::rendering::vk_renderer::{Renderer, WindowSpecification};
use winit::{
    self,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
};

pub struct Application<'a> {
    running: bool,
    window: WindowSpecification<'a>,
    layers: Vec<LayerRef>,
    last_time: std::time::Instant,

    renderer: Renderer,
}

impl<'a> Application<'a> {
    pub fn new() -> Application<'a> {
        Application {
            running: false,
            window: WindowSpecification {
                name: "Morigu app",
                version: (1, 0, 0),
                width: 1280,
                height: 720,
            },
            layers: Vec::new(),
            last_time: std::time::Instant::now(),
            renderer: Renderer::new(),
        }
    }

    pub fn from_spec(spec: WindowSpecification<'a>) -> Application<'a> {
        Application {
            running: false,
            window: spec,
            layers: Vec::new(),
            last_time: std::time::Instant::now(),
            renderer: Renderer::new(),
        }
    }

    pub fn run(&mut self, base_layer: LayerRef) {
        self.running = true;
        let mut event_loop = EventLoop::new();
        self.renderer.init(&self.window, &event_loop);

        self.push_layer(base_layer);
        event_loop.run_return(move |event, _, control_flow| {
            let transition = match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => Transition::Shutdown,
                Event::WindowEvent { event: _, .. } => self
                    .layers
                    .last()
                    .expect("LayerStack cannot be empty!")
                    .on_event(&event),
                Event::MainEventsCleared => {
                    let new_time = std::time::Instant::now();
                    let ts =
                        Timestep::from_nano(new_time.duration_since(self.last_time).as_nanos());
                    self.last_time = new_time;
                    self.layers
                        .last()
                        .expect("LayerStack cannot be empty!")
                        .on_update(ts.clone())
                }
                _ => Transition::None,
            };

            self.handle_transition(transition, control_flow);
        });
    }

    fn handle_transition(&mut self, trans: Transition, control_flow: &mut ControlFlow) {
        match trans {
            Transition::Push(new_layer) => self.push_layer(new_layer),
            Transition::Pop => self.pop_layer(),
            Transition::Switch(new_layer) => {
                self.pop_layer();
                self.push_layer(new_layer);
            }
            Transition::Shutdown => *control_flow = ControlFlow::Exit,
            Transition::None => (),
        }
    }

    fn push_layer(&mut self, new_layer: LayerRef) {
        match self.layers.last() {
            Some(layer) => layer.on_pause(),
            None => (),
        }
        self.layers.push(new_layer);
        self.layers.last().unwrap().on_attach();
    }

    fn pop_layer(&mut self) {
        self.layers
            .last()
            .expect("Could not pop layer: layer stack is empty!")
            .on_detach();

        self.layers.pop();
        match self.layers.last() {
            Some(layer) => layer.on_resume(),
            None => (),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}
