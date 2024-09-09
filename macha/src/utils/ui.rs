use morrigu::egui;

use super::startup_state::SwitchableStates;

pub fn draw_debug_utils(ctx: &egui::Context, dt: std::time::Duration, current_state: &mut SwitchableStates) {
    egui::Window::new("Debug tools").show(ctx, |ui| {
        let color = match dt.as_millis() {
            0..=25 => [51, 204, 51],
            26..=50 => [255, 153, 0],
            _ => [204, 51, 51],
        };
        ui.colored_label(
            egui::Color32::from_rgb(color[0], color[1], color[2]),
            format!("FPS: {} ({}ms)", 1.0 / dt.as_secs_f32(), dt.as_millis()),
        );

        egui::ComboBox::from_label("Select desired state:")
            .selected_text(format!("{current_state}"))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    current_state,
                    SwitchableStates::Editor,
                    format!("{}", SwitchableStates::Editor),
                );
                ui.selectable_value(
                    current_state,
                    SwitchableStates::GLTFLoader,
                    format!("{}", SwitchableStates::GLTFLoader),
                );
                ui.selectable_value(
                    current_state,
                    SwitchableStates::CSTest,
                    format!("{}", SwitchableStates::CSTest),
                );
                ui.selectable_value(
                    current_state,
                    SwitchableStates::PBRTest,
                    format!("{}", SwitchableStates::PBRTest),
                );

                #[cfg(feature = "ray_tracing")]
                ui.selectable_value(
                    current_state,
                    SwitchableStates::RTTest,
                    format!("{}", SwitchableStates::RTTest),
                );
            });
    });
}
