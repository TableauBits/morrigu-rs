use bevy_ecs::prelude::{Query, Res};
use egui::LayerId;
use egui_gizmo::Gizmo;
use morrigu::components::{camera::Camera, transform::Transform};
use nalgebra_glm as glm;

use crate::components::selected_entity::SelectedEntity;

pub fn draw_gizmo(
    mut query: Query<(&mut Transform, &mut SelectedEntity)>,
    camera: Res<Camera>,
    egui_context: Res<egui::Context>,
) {
    for (mut transform, _) in query.iter_mut() {
        egui::Area::new("Gizmo viewport")
            .fixed_pos((0.0, 0.0))
            .show(&egui_context, |ui| {
                ui.with_layer_id(LayerId::background(), |ui| {
                    let gizmo = Gizmo::new("Selected entity gizmo")
                        .view_matrix(*camera.view())
                        .projection_matrix(*camera.projection())
                        .model_matrix(*transform.matrix())
                        .mode(egui_gizmo::GizmoMode::Translate);

                    if let Some(response) = gizmo.interact(ui) {
                        let vec = response.transform.to_vec();
                        let vec = vec
                            .iter()
                            .flat_map(|slice| slice.to_vec())
                            .collect::<Vec<_>>();
                        transform.set_matrix(&glm::make_mat4(&vec));
                    }
                });
            });
    }
}
