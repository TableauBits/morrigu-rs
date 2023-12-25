use morrigu::application::{ApplicationState, BuildableApplicationState};

pub struct RayTracerState {}

impl BuildableApplicationState<()> for RayTracerState {
    fn build(_context: &mut morrigu::application::StateContext, _: ()) -> Self {
        Self {}
    }
}

impl ApplicationState for RayTracerState {
    fn on_attach(&mut self, _context: &mut morrigu::application::StateContext) {}

    fn on_update(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::StateContext,
    ) {
    }

    fn after_systems(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::StateContext,
    ) {
    }

    fn on_update_egui(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::EguiUpdateContext,
    ) {
    }

    fn after_ui_systems(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::EguiUpdateContext,
    ) {
    }

    fn on_event(
        &mut self,
        _event: winit::event::Event<()>,
        _context: &mut morrigu::application::StateContext,
    ) {
    }

    fn on_drop(&mut self, _context: &mut morrigu::application::StateContext) {}
}
