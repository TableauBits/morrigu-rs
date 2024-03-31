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
