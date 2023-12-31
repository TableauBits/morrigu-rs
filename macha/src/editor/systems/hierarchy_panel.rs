use bevy_ecs::prelude::{Entity, Query, Res, ResMut};
use morrigu::{components::resource_wrapper::ResourceWrapper, egui};

use egui::collapsing_header::CollapsingState;

use crate::editor::{
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

    let id = ui.make_persistent_id(format!("EntityList.{}", entity.index()));
    CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| {
            if ui.selectable_label(is_selected, &options.name).clicked() && !is_selected {
                ecs_buffer.command_buffer.push(ECSJob::SelectEntity {
                    entity: Some(entity),
                });
            }
        })
        .body(|ui| {
            ui.label("nothing to see here");
        });
}

#[allow(dead_code)]
pub fn draw_hierarchy_panel(
    query: Query<(Entity, &MachaEntityOptions, Option<&SelectedEntity>)>,
    egui_context: Res<ResourceWrapper<egui::Context>>,
    mut ecs_buffer: ResMut<ECSBuffer>,
) {
    egui::Window::new("Entity list").show(&egui_context.data, |ui| {
        for entity_info in query.iter() {
            draw_single_entity(entity_info, ui, &mut ecs_buffer);
        }
    });
}

pub fn draw_hierarchy_panel_stable(
    query: Query<(Entity, &MachaEntityOptions, Option<&SelectedEntity>)>,
    egui_context: Res<ResourceWrapper<egui::Context>>,
    mut ecs_buffer: ResMut<ECSBuffer>,
) {
    let iter = query.iter();
    let hint = iter.size_hint();

    let mut stable_entity_list = vec![];
    if let Some(upper_bound) = hint.1 {
        stable_entity_list.reserve(upper_bound)
    }
    for element in iter {
        stable_entity_list.push(element);
    }
    stable_entity_list.sort_by(|element1, element2| element1.0.cmp(&element2.0));

    if let Some(window_sense) =
        egui::Window::new("Entity list (stable)").show(&egui_context.data, |ui| {
            ui.label(format!("count hint: {:?}", hint.1));
            for entity_info in stable_entity_list {
                draw_single_entity(entity_info, ui, &mut ecs_buffer);
            }
        })
    {
        if window_sense.response.clicked() {
            ecs_buffer
                .command_buffer
                .push(ECSJob::SelectEntity { entity: None });
        }
    }
}
