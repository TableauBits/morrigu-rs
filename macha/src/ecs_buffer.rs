use bevy_ecs::prelude::{Component, Entity};

#[non_exhaustive]
pub enum ECSJob {
    SelectEntity { entity: Option<Entity> },
}

#[derive(Component, Default)]
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
