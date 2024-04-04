use std::fmt::Display;

use morrigu::egui;

#[derive(PartialEq, Copy, Clone)]
pub enum SwitchableStates {
    Editor,
    GLTFLoader,
    CSTest,
    RTTest,
}

impl Display for SwitchableStates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            SwitchableStates::Editor => "Macha Editor",
            SwitchableStates::GLTFLoader => "GLTF Loader and Viewer",
            SwitchableStates::CSTest => "Compute Shader Test",
            SwitchableStates::RTTest => "Ray Tracing Test",
        };

        write!(f, "{}", name)
    }
}

pub fn draw_state_switcher(ctx: &egui::Context, current_state: &mut SwitchableStates) {
    egui::Window::new("State Switcher").show(ctx, |ui| {
        egui::ComboBox::from_label("Select desired state:")
            .selected_text(format!("{current_state}"))
            .show_ui(ui, |ui| {
                ui.selectable_value(current_state, SwitchableStates::Editor, format!("{}", SwitchableStates::Editor));
                ui.selectable_value(current_state, SwitchableStates::GLTFLoader, format!("{}", SwitchableStates::GLTFLoader));
                ui.selectable_value(current_state, SwitchableStates::CSTest, format!("{}", SwitchableStates::CSTest));
                ui.selectable_value(current_state, SwitchableStates::RTTest, format!("{}", SwitchableStates::RTTest));
            });
    });
}

pub fn draw_debug_utils(ctx: &egui::Context, dt: std::time::Duration) {
    egui::Window::new("Debug info").show(ctx, |ui| {
        let color = match dt.as_millis() {
            0..=25 => [51, 204, 51],
            26..=50 => [255, 153, 0],
            _ => [204, 51, 51],
        };
        ui.colored_label(
            egui::Color32::from_rgb(color[0], color[1], color[2]),
            format!("FPS: {} ({}ms)", 1.0 / dt.as_secs_f32(), dt.as_millis()),
        );
    });
}
