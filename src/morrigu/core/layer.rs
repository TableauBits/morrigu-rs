use super::timestep::Timestep;
use winit::event::Event;

pub trait Layer {
    fn on_attach(&self) {}
    fn on_detach(&self) {}

    fn on_update(&self, _: Timestep) {}
    fn on_event(&self, _: &Event<()>) {}
}
