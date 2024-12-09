use morrigu::bevy_ecs::prelude::{Query, Res};
use morrigu::bevy_ecs::system::ResMut;
use morrigu::winit_input_helper::WinitInputHelper;
use morrigu::{
    components::{camera::Camera, resource_wrapper::ResourceWrapper, transform::Transform},
    egui,
    math_types::Mat4,
};

use egui::LayerId;
use transform_gizmo::GizmoVisuals;
use transform_gizmo_egui::GizmoExt;

use crate::editor::components::{
    macha_options::MachaGlobalOptions, selected_entity::SelectedEntity,
};

// This is the big problem with this library:
// https://github.com/urholaukkarinen/transform-gizmo/issues/19
pub fn draw_gizmo(
    mut query: Query<(&mut Transform, &mut SelectedEntity)>,
    camera: Res<Camera>,
    mut macha_options: ResMut<MachaGlobalOptions>,
    egui_context: Res<ResourceWrapper<egui::Context>>,
    window_input: Res<ResourceWrapper<WinitInputHelper>>,
) {
    let window_input = &window_input.data;
    for (mut transform, _) in query.iter_mut() {
        egui::Area::new("Gizmo viewport".into())
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

                    let mut config = *macha_options.gizmo.config();
                    config.view_matrix = (*camera.view()).as_dmat4().into();
                    config.projection_matrix = (*camera.projection()).as_dmat4().into();
                    config.viewport = egui::Rect::EVERYTHING;
                    config.snapping = is_snapping_enabled;
                    config.visuals = visuals;
                    macha_options.gizmo.update_config(config);

                    if let Some((_, new_transforms)) = macha_options.gizmo.interact(
                        ui,
                        &[
                            transform_gizmo::math::Transform::from_scale_rotation_translation(
                                transform.scale().as_dvec3(),
                                transform.rotation().as_dquat(),
                                transform.translation().as_dvec3(),
                            ),
                        ],
                    ) {
                        let new_transform = new_transforms[0];
                        let scale_mrg: morrigu::glam::DVec3 = new_transform.scale.into();
                        let rotation_mrg: morrigu::glam::DQuat = new_transform.rotation.into();
                        let translation_mrg: morrigu::glam::DVec3 =
                            new_transform.translation.into();
                        *transform = Mat4::from_scale_rotation_translation(
                            scale_mrg.as_vec3(),
                            rotation_mrg.as_quat(),
                            translation_mrg.as_vec3(),
                        )
                        .into();
                    }
                });
            });
    }
}
