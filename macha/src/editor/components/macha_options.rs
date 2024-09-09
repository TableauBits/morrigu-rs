use morrigu::{
    bevy_ecs::{self, prelude::Component, system::Resource},
    egui,
};
use transform_gizmo::{Gizmo, GizmoConfig, GizmoMode};

#[derive(Component)]
pub struct MachaEntityOptions {
    pub name: String,
}

#[derive(Resource)]
pub struct MachaGlobalOptions {
    pub gizmo: Gizmo,
}

impl MachaGlobalOptions {
    pub fn new() -> Self {
        Self {
            gizmo: Gizmo::new(GizmoConfig {
                viewport: egui::Rect::EVERYTHING,
                modes: GizmoMode::all_translate(),
                snap_angle: f32::to_radians(45.0),
                snap_distance: 0.5,
                snap_scale: 0.5,
                ..Default::default()
            }),
        }
    }
}

impl Default for MachaGlobalOptions {
    fn default() -> Self {
        Self::new()
    }
}
