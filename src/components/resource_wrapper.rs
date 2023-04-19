use std::ops::{Deref, DerefMut};

use bevy_ecs::system::Resource;

#[derive(Resource)]
pub struct ResourceWrapper<T> {
    pub data: T,
}

impl<T> ResourceWrapper<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T> Deref for ResourceWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ResourceWrapper<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
