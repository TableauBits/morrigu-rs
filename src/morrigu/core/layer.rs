use super::timestep::Timestep;
use winit::event::Event;

pub type LayerRef = Box<dyn Layer>;

pub enum Transition {
    Push(LayerRef),
    Pop,
    Switch(LayerRef),
    Shutdown,
    None,
}

pub trait Layer {
    fn on_attach(&self) {}
    fn on_detach(&self) {}

    fn on_pause(&self) {}
    fn on_resume(&self) {}

    fn on_update(&self, _: Timestep) -> Transition {
        Transition::None
    }
    fn on_event(&self, _: &Event<()>) -> Transition {
        Transition::None
    }
}
