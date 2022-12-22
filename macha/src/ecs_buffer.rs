use bevy_ecs::{
    prelude::{Component, Entity},
    system::Resource,
};

#[non_exhaustive]
pub enum ECSJob {
    SelectEntity { entity: Option<Entity> },
}

#[derive(Component, Default, Resource)]
pub struct ECSBuffer {
    pub command_buffer: Vec<ECSJob>,
}

impl ECSBuffer {
    pub fn new() -> Self {
        Self {
            command_buffer: Vec::new(),
        }
    }
}
