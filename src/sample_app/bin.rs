use morrigu::{
    self,
    core::{
        application::Application,
        layer::{Layer, Transition},
    },
};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};

struct SampleLayerA;
struct SampleLayerB;

impl Layer for SampleLayerA {
    fn on_attach(&self) {
        println!("Layer A attached");
    }

    fn on_event(&self, event: &Event<()>) -> Transition {
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Space),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => Transition::Switch(Box::new(SampleLayerB {})),
            _ => Transition::None,
        }
    }
}

impl Layer for SampleLayerB {
    fn on_attach(&self) {
        println!("Layer B attached");
    }

    fn on_event(&self, event: &Event<()>) -> Transition {
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(key),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => match key {
                VirtualKeyCode::Space => Transition::Switch(Box::new(SampleLayerA {})),
                VirtualKeyCode::Escape => Transition::Shutdown,
                _ => Transition::None,
            },
            _ => Transition::None,
        }
    }
}

pub fn main() {
    let mut test = Application::new(Box::new(SampleLayerA {}));
    test.run();
}
