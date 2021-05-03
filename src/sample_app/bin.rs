use morrigu::{
    self,
    core::{application::Application, layer::Layer, timestep::Timestep},
};
use winit::event::Event;

struct SampleLayer;

impl Layer for SampleLayer {
    fn on_update(&self, ts: Timestep) {
        println!("SampleLayer updated ({})!", ts.as_seconds())
    }

    fn on_event(&self, _: &Event<()>) {
        println!("SampleLayer received event!")
    }
}

pub fn main() {
    let mut test = Application::new();
    test.push_layer(Box::new(SampleLayer {}));
    test.run();
}
