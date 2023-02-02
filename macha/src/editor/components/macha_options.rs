use bevy_ecs::{prelude::Component, system::Resource};

#[derive(Component)]
pub struct MachaEntityOptions {
    pub name: String,
}

#[derive(Resource)]
pub struct MachaGlobalOptions {
    pub preferred_gizmo: egui_gizmo::GizmoMode,
}

impl MachaGlobalOptions {
    pub fn new() -> Self {
        Self {
            preferred_gizmo: egui_gizmo::GizmoMode::Translate,
        }
    }
}

impl Default for MachaGlobalOptions {
    fn default() -> Self {
        Self::new()
    }
}
