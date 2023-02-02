use bevy_ecs::prelude::{Query, Res};
use egui::LayerId;
use egui_gizmo::{Gizmo, GizmoVisuals};
use morrigu::{
    components::{camera::Camera, resource_wrapper::ResourceWrapper, transform::Transform},
    vector_type::Mat4,
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
                        .view_matrix(*camera.view())
                        .projection_matrix(*camera.projection())
                        .model_matrix(*transform.matrix())
                        .mode(macha_options.preferred_gizmo)
                        .visuals(visuals)
                        .snap_distance(0.5)
                        .snap_angle(f32::to_radians(45.0))
                        .snap_scale(0.5)
                        .snapping(is_snapping_enabled);

                    if let Some(response) = gizmo.interact(ui) {
                        let vec = response.transform.to_vec();
                        let vec = vec
                            .iter()
                            .flat_map(|slice| slice.to_vec())
                            .collect::<Vec<_>>();
                        transform.set_matrix(&Mat4::from_column_slice(&vec));
                    }
                });
            });
    }
}
