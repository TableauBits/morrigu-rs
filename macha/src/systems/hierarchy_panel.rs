use bevy_ecs::prelude::{Entity, Query, Res, ResMut};
use egui::collapsing_header::CollapsingState;

use crate::{
    components::{macha_options::MachaEntityOptions, selected_entity::SelectedEntity},
    ecs_buffer::{ECSBuffer, ECSJob},
};

pub fn draw_hierarchy_panel(
    query: Query<(Entity, &MachaEntityOptions, Option<&SelectedEntity>)>,
    egui_context: Res<egui::Context>,
    mut ecs_buffer: ResMut<ECSBuffer>,
) {
    egui::Window::new("Entity list").show(&egui_context, |ui| {
        for (entity, options, is_selected) in query.iter() {
            let is_selected = is_selected.is_some();
            let id = ui.make_persistent_id(format!("EntityList.{}", entity.id()));
            CollapsingState::load_with_default_open(ui.ctx(), id, false)
                .show_header(ui, |ui| {
                    if ui.selectable_label(is_selected, &options.name).clicked() && !is_selected {
                        ecs_buffer
                            .command_buffer
                            .push(ECSJob::SelectEntity { entity });
                    }
                })
                .body(|ui| {
                    ui.label("nothing to see here");
                });
        }
    });
}
