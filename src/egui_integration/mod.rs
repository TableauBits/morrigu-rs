mod painter;
pub use painter::Painter;

use crate::renderer::Renderer;

use self::painter::PainterCreationError;

pub struct EguiIntegration {
    pub egui_platform_state: egui_winit::State,
    pub painter: Painter,

    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
}

impl EguiIntegration {
    pub fn new(
        window: &winit::window::Window,
        renderer: &mut Renderer,
    ) -> Result<Self, PainterCreationError> {
        let painter = Painter::new(renderer)?;
        let context = egui::Context::default();
        let egui_platform_state = egui_winit::State::new(context.clone(), egui::ViewportId::ROOT, window, None, None);

        Ok(Self {
            egui_platform_state,
            painter,
            shapes: vec![],
            textures_delta: Default::default(),
        })
    }

    pub fn handle_event(&mut self, window: &winit::window::Window, event: &winit::event::Event<()>) -> bool {
        match event {
            winit::event::Event::WindowEvent {
                window_id: _,
                event,
            } => {
                self.egui_platform_state
                    .on_window_event(window, event)
                    .consumed
            }

            _ => false,
        }
    }

    pub fn run(
        &mut self,
        window: &winit::window::Window,
        ui_callback: impl FnMut(&egui::Context),
    ) {
        let raw_input = self.egui_platform_state.take_egui_input(window);
        let egui::FullOutput {
            platform_output,
            textures_delta,
            shapes,
            ..
        } =
        self.egui_platform_state.egui_ctx().run(raw_input, ui_callback);

        self.egui_platform_state
            .handle_platform_output(window, platform_output);
        self.shapes = shapes;
        self.textures_delta.append(textures_delta);
    }

    pub fn paint(&mut self, renderer: &mut Renderer) {
        let shapes = std::mem::take(&mut self.shapes);
        let clipped_primitives = self.egui_platform_state.egui_ctx().tessellate(shapes, self.egui_platform_state.egui_ctx().pixels_per_point());
        let textures_delta = std::mem::take(&mut self.textures_delta);

        self.painter.paint_and_update_textures(
            self.egui_platform_state.egui_ctx().pixels_per_point(),
            &clipped_primitives,
            textures_delta,
            renderer,
        );
    }
}
