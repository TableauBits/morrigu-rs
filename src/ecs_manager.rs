use std::time::Instant;

use bevy_ecs::{
    prelude::World,
    schedule::{Schedule, Stage},
};

use crate::{components::camera::Camera, renderer::Renderer, utils::ThreadSafeRef};

pub struct ECSManager {
    pub world: World,
    pub resize_callback: Option<Box<dyn Fn(u32, u32)>>,
    systems_schedule: Schedule,
}

impl ECSManager {
    pub(crate) fn new(renderer_ref: &ThreadSafeRef<Renderer>, camera: Camera) -> Self {
        let renderer_ref = ThreadSafeRef::clone(renderer_ref);

        let mut world = World::new();
        let systems_schedule = bevy_ecs::schedule::Schedule::default();

        world.insert_resource(camera);
        world.insert_resource(Instant::now());
        world.insert_resource(renderer_ref);

        Self {
            world,
            resize_callback: None,
            systems_schedule,
        }
    }

    pub(crate) fn on_resize(&mut self, width: u32, height: u32) {
        let mut camera = self
            .world
            .get_resource_mut::<Camera>()
            .expect("No camera bound to world");
        camera.on_resize(width, height);

        if let Some(callback) = self.resize_callback.as_ref() {
            callback(width, height);
        }
    }

    pub fn redefine_systems_schedule<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Schedule),
    {
        let mut new_schedule = Schedule::default();

        f(&mut new_schedule);

        self.systems_schedule = new_schedule;
    }

    pub(crate) fn run_schedule(&mut self) {
        self.systems_schedule.run(&mut self.world);
    }
}
