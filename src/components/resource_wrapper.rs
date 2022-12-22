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
