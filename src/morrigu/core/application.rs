use super::layer::Layer;
use super::timestep::Timestep;
use winit::{
    self,
    dpi::PhysicalSize,
    event::Event,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
};

pub struct WindowSpecification<'a> {
    pub name: &'a str,
    pub width: u32,
    pub height: u32,
}

pub struct Application<'a> {
    running: bool,
    window: WindowSpecification<'a>,
    layers: Vec<Box<dyn Layer>>,
    last_time: std::time::Instant,
}

impl<'a> Application<'a> {
    pub fn new() -> Application<'a> {
        Application {
            running: false,
            window: WindowSpecification {
                name: "Morigu app",
                width: 1280,
                height: 720,
            },
            layers: Vec::new(),
            last_time: std::time::Instant::now(),
        }
    }

    pub fn from_spec(spec: WindowSpecification<'a>) -> Application<'a> {
        Application {
            running: false,
            window: spec,
            layers: Vec::new(),
            last_time: std::time::Instant::now(),
        }
    }

    pub fn run(&mut self) {
        self.running = true;
        let mut event_loop = EventLoop::new();
        let _window = winit::window::WindowBuilder::new()
            .with_title(self.window.name)
            .with_inner_size(PhysicalSize {
                width: self.window.width,
                height: self.window.height,
            })
            .build(&event_loop);

        event_loop.run_return(move |event, _, control_flow| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent { event: _, .. } => {
                for layer in self.layers.iter() {
                    layer.on_event(&event);
                }
            }
            Event::MainEventsCleared => {
                let new_time = std::time::Instant::now();
                let ts = Timestep::from_nano(new_time.duration_since(self.last_time).as_nanos());
                for layer in self.layers.iter() {
                    layer.on_update(ts.clone())
                }
                self.last_time = new_time;
            }
            _ => (),
        });
    }

    pub fn push_layer(&mut self, new_layer: Box<dyn Layer>) {
        self.layers.push(new_layer);
        self.layers.last().unwrap().on_attach();
    }

    pub fn pop_layer(&mut self) -> Box<dyn Layer> {
        self.layers
            .last()
            .expect("Could not pop layer: layer stack is empty!")
            .on_detach();

        self.layers.pop().unwrap()
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}
