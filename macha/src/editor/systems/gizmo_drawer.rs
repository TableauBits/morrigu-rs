use bevy_ecs::prelude::{Query, Res};
use egui::LayerId;
use egui_gizmo::{Gizmo, GizmoVisuals};
use morrigu::{
    components::{camera::Camera, resource_wrapper::ResourceWrapper, transform::Transform},
    math_types::Mat4,
};
use winit_input_helper::WinitInputHelper;

use crate::editor::components::{
    macha_options::MachaGlobalOptions, selected_entity::SelectedEntity,
};

pub fn draw_gizmo(
    mut query: Query<(&mut Transform, &mut SelectedEntity)>,
    camera: Res<Camera>,
    macha_options: Res<MachaGlobalOptions>,
    egui_context: Res<ResourceWrapper<egui::Context>>,
    window_input: Res<ResourceWrapper<WinitInputHelper>>,
) {
    let window_input = &window_input.data;
    for (mut transform, _) in query.iter_mut() {
        egui::Area::new("Gizmo viewport")
            .fixed_pos((0.0, 0.0))
            .show(&egui_context.data, |ui| {
                ui.with_layer_id(LayerId::background(), |ui| {
                    let is_snapping_enabled = window_input.held_control();

                    let size = camera.size();
                    let scaling = if size.x < size.y {
                        size.x / 1280.0
                    } else {
                        size.y / 720.0
                    };
                    let mut visuals = GizmoVisuals::default();
                    visuals.gizmo_size *= 1.2 * scaling;
                    visuals.stroke_width *= 1.2 * (((scaling - 1.0) * 0.3) + 1.0);
                    visuals.inactive_alpha += 0.25;

                    let gizmo = Gizmo::new("Selected entity gizmo")
                        .view_matrix(camera.view().to_cols_array_2d())
                        .projection_matrix(camera.projection().to_cols_array_2d())
                        .model_matrix((*transform).into())
                        .mode(macha_options.preferred_gizmo)
                        .visuals(visuals)
                        .snap_distance(0.5)
                        .snap_angle(f32::to_radians(45.0))
                        .snap_scale(0.5)
                        .snapping(is_snapping_enabled);

                    if let Some(response) = gizmo.interact(ui) {
                        *transform =
                            Mat4::from_cols_array_2d(&response.transform_cols_array_2d()).into();
                    }
                });
            });
    }
}
