use std::time::Instant;

use bevy_ecs::{prelude::World, schedule::Schedule};

use crate::{
    components::{camera::Camera, resource_wrapper::ResourceWrapper},
    renderer::Renderer,
    utils::ThreadSafeRef,
};

pub struct ECSManager {
    pub world: World,
    pub resize_callback: Option<Box<dyn Fn(u32, u32)>>,

    systems_schedule: Schedule,
    #[cfg(feature = "egui")]
    ui_systems_schedule: Schedule,
}

impl ECSManager {
    pub(crate) fn new(renderer_ref: &ThreadSafeRef<Renderer>, camera: Camera) -> Self {
        let renderer_ref = ThreadSafeRef::clone(renderer_ref);

        let mut world = World::new();
        let systems_schedule = bevy_ecs::schedule::Schedule::default();
        #[cfg(feature = "egui")]
        let ui_systems_schedule = bevy_ecs::schedule::Schedule::default();

        world.insert_resource(camera);
        world.insert_resource(ResourceWrapper::new(Instant::now()));
        world.insert_resource(renderer_ref);

        #[cfg(feature = "egui")]
        {
            Self {
                world,
                resize_callback: None,
                systems_schedule,
                ui_systems_schedule,
            }
        }

        #[cfg(not(feature = "egui"))]
        {
            Self {
                world,
                resize_callback: None,
                systems_schedule,
            }
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

    #[cfg(feature = "egui")]
    pub fn redefine_ui_systems_schedule<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Schedule),
    {
        let mut new_ui_schedule = Schedule::default();

        f(&mut new_ui_schedule);

        self.ui_systems_schedule = new_ui_schedule;
    }

    #[cfg(feature = "egui")]
    pub(crate) fn run_ui_schedule(&mut self, egui_context: &egui::Context) {
        self.world
            .insert_resource(ResourceWrapper::new(egui_context.clone()));
        self.ui_systems_schedule.run(&mut self.world);
        self.world
            .remove_resource::<ResourceWrapper<egui::Context>>();
    }
}
