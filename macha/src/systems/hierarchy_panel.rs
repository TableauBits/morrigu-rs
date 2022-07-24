use bevy_ecs::prelude::{Entity, Query, Res, ResMut};
use egui::collapsing_header::CollapsingState;

use crate::{
    components::{macha_options::MachaEntityOptions, selected_entity::SelectedEntity},
    ecs_buffer::{ECSBuffer, ECSJob},
};

fn draw_single_entity(
    infos: (Entity, &MachaEntityOptions, Option<&SelectedEntity>),
    ui: &mut egui::Ui,
    ecs_buffer: &mut ECSBuffer,
) {
    let entity = infos.0;
    let options = infos.1;
    let is_selected = infos.2.is_some();

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

#[allow(dead_code)]
pub fn draw_hierarchy_panel(
    query: Query<(Entity, &MachaEntityOptions, Option<&SelectedEntity>)>,
    egui_context: Res<egui::Context>,
    mut ecs_buffer: ResMut<ECSBuffer>,
) {
    egui::Window::new("Entity list").show(&egui_context, |ui| {
        for entity_info in query.iter() {
            draw_single_entity(entity_info, ui, &mut ecs_buffer);
        }
    });
}

pub fn draw_hierarchy_panel_stable(
    query: Query<(Entity, &MachaEntityOptions, Option<&SelectedEntity>)>,
    egui_context: Res<egui::Context>,
    mut ecs_buffer: ResMut<ECSBuffer>,
) {
    let mut stable_entity_list = vec![];
    for element in query.iter() {
        stable_entity_list.push(element);
    }
    stable_entity_list.sort_by(|element1, element2| element1.0.cmp(&element2.0));

    egui::Window::new("Entity list (stable)").show(&egui_context, |ui| {
        for entity_info in stable_entity_list {
            draw_single_entity(entity_info, ui, &mut ecs_buffer);
        }
    });
}
