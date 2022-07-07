mod painter;
pub use painter::Painter;

use crate::{error::Error, renderer::Renderer};

pub struct EguiIntegration {
    pub egui_context: egui::Context,
    pub egui_platform_state: egui_winit::State,
    pub painter: Painter,

    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
}

impl EguiIntegration {
    pub fn new(window: &winit::window::Window, renderer: &mut Renderer) -> Result<Self, Error> {
        let painter = Painter::new(renderer)?;
        let egui_platform_state = egui_winit::State::new(painter.max_texture_size, window);

        Ok(Self {
            egui_context: Default::default(),
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
            } => self.egui_platform_state.on_event(&self.egui_context, event),

            _ => false,
        }
    }

    pub fn run(
        &mut self,
        window: &winit::window::Window,
        ui_callback: impl FnMut(&egui::Context),
    ) -> bool {
        let raw_input = self.egui_platform_state.take_egui_input(window);
        let egui::FullOutput {
            platform_output,
            needs_repaint,
            textures_delta,
            shapes,
        } = self.egui_context.run(raw_input, ui_callback);

        self.egui_platform_state.handle_platform_output(
            window,
            &self.egui_context,
            platform_output,
        );
        self.shapes = shapes;
        self.textures_delta.append(textures_delta);

        needs_repaint
    }

    pub fn paint(&mut self, renderer: &mut Renderer) {
        let shapes = std::mem::take(&mut self.shapes);
        let clipped_primitives = self.egui_context.tessellate(shapes);
        let textures_delta = std::mem::take(&mut self.textures_delta);

        self.painter.paint_and_update_textures(
            self.egui_context.pixels_per_point(),
            &clipped_primitives,
            textures_delta,
            renderer,
        );
    }
}
