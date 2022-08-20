mod painter;
pub use painter::Painter;

use crate::{error::Error, renderer::Renderer};

pub struct EguiIntegration {
    pub context: egui::Context,
    pub egui_platform_state: egui_winit::State,
    pub painter: Painter,

    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
}

impl EguiIntegration {
    pub fn new(
        event_loop: &winit::event_loop::EventLoopWindowTarget<()>,
        renderer: &mut Renderer,
    ) -> Result<Self, Error> {
        let painter = Painter::new(renderer)?;
        let egui_platform_state = egui_winit::State::new(event_loop);

        Ok(Self {
            context: Default::default(),
            egui_platform_state,
            painter,
            shapes: vec![],
            textures_delta: Default::default(),
        })
    }

    pub fn handle_event(&mut self, event: &winit::event::Event<()>) -> bool {
        match event {
            winit::event::Event::WindowEvent {
                window_id: _,
                event,
            } => self.egui_platform_state.on_event(&self.context, event),

            _ => false,
        }
    }

    pub fn run(
        &mut self,
        window: &winit::window::Window,
        ui_callback: impl FnMut(&egui::Context),
    ) -> std::time::Duration {
        let raw_input = self.egui_platform_state.take_egui_input(window);
        let egui::FullOutput {
            platform_output,
            repaint_after,
            textures_delta,
            shapes,
        } = self.context.run(raw_input, ui_callback);

        self.egui_platform_state
            .handle_platform_output(window, &self.context, platform_output);
        self.shapes = shapes;
        self.textures_delta.append(textures_delta);

        repaint_after
    }

    pub fn paint(&mut self, renderer: &mut Renderer) {
        let shapes = std::mem::take(&mut self.shapes);
        let clipped_primitives = self.context.tessellate(shapes);
        let textures_delta = std::mem::take(&mut self.textures_delta);

        self.painter.paint_and_update_textures(
            self.context.pixels_per_point(),
            &clipped_primitives,
            textures_delta,
            renderer,
        );
    }
}
