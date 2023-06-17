mod camera;
mod loader;
mod scene;

use std::{iter::zip, path::Path};

use morrigu::{
    application::{ApplicationState, BuildableApplicationState, EguiUpdateContext, Event},
    components::{
        camera::{Camera, PerspectiveData},
        transform::Transform,
    },
    descriptor_resources::DescriptorResources,
    math_types::{Quat, Vec2, Vec3, Vec4},
    shader::Shader,
    systems::mesh_renderer,
};

use self::{
    camera::ViewerCamera,
    loader::LightData,
    scene::{Material, Scene, Vertex},
};

pub struct GLTFViewerState {
    light_data: LightData,
    camera: ViewerCamera,
    scene: Scene,
}

impl BuildableApplicationState<()> for GLTFViewerState {
    fn build(context: &mut morrigu::application::StateContext, _: ()) -> Self {
        let mut camera = Camera::builder().build(
            morrigu::components::camera::Projection::Perspective(PerspectiveData {
                horizontal_fov: f32::to_radians(50.0),
                near_plane: 0.001,
                far_plane: 1000.0,
            }),
            &Vec2::new(1280.0, 720.0),
        );
        camera.set_position(&Vec3::new(0.0, 0.0, 3.0));

        let camera = ViewerCamera::new(camera);

        let pbr_shader = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/pbr/pbr.vert"),
            include_bytes!("shaders/gen/pbr/pbr.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create pbr shader");

        let default_shader = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/default/default.vert"),
            include_bytes!("shaders/gen/default/default.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create default shader");
        let default_material = Material::builder()
            .build(
                &default_shader,
                DescriptorResources::empty(),
                context.renderer,
            )
            .expect("Failed to create default material");

        let scene = loader::load_gltf(
            Path::new("assets/scenes/sponza/Sponza.gltf"),
            // Transform::default(),
            Transform::from_trs(
                &Vec3::default(),
                &Quat::default(),
                &Vec3::new(100.0, 100.0, 100.0),
                // &Vec3::default(),
            ),
            pbr_shader,
            context.renderer.default_texture(),
            default_material,
            context.renderer,
        )
        .expect("Failed to load GLTF scene");

        for (transform, mesh_rendering_ref) in zip(&scene.transforms, &scene.mesh_renderings) {
            context
                .ecs_manager
                .world
                .spawn((transform.clone(), mesh_rendering_ref.clone()));
        }

        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_system(mesh_renderer::render_meshes::<Vertex>);
        });

        let light_data = LightData {
            light_direction: Vec4::new(-1.0, -1.0, 0.0, 0.0).normalize(),
            light_color: Vec4::new(0.7, 0.2, 0.2, 1.0),
            ambient_light_color: Vec3::new(0.3, 0.3, 0.3),
            ambient_light_intensity: 0.2,
            camera_position: Vec4::new(0.0, 0.0, 3.0, 0.0),
        };

        for material in &scene.materials {
            material
                .lock()
                .update_uniform(0, light_data)
                .expect("Failed to update light data to material");
        }

        Self {
            light_data,
            camera,
            scene,
        }
    }
}

impl ApplicationState for GLTFViewerState {
    fn on_attach(&mut self, _context: &mut morrigu::application::StateContext) {}

    fn on_update(
        &mut self,
        dt: std::time::Duration,
        context: &mut morrigu::application::StateContext,
    ) {
        self.camera.on_update(dt, context.window_input_state);

        let cam_pos = self.camera.mrg_camera.position();
        self.light_data.camera_position = Vec4::new(cam_pos.x, cam_pos.y, cam_pos.z, 0.0);

        for material in &self.scene.materials {
            material
                .lock()
                .update_uniform(0, self.light_data)
                .expect("Failed to update light data");
        }

        context
            .ecs_manager
            .world
            .insert_resource(self.camera.mrg_camera);
    }

    fn on_event(&mut self, event: Event<()>, _context: &mut morrigu::application::StateContext) {
        #[allow(clippy::single_match)] // Temporary
        match event {
            morrigu::application::Event::WindowEvent {
                event:
                    winit::event::WindowEvent::Resized(winit::dpi::PhysicalSize {
                        width, height, ..
                    }),
                ..
            } => {
                self.camera.on_resize(width, height);
            }
            _ => (),
        }
    }

    fn on_update_egui(&mut self, dt: std::time::Duration, context: &mut EguiUpdateContext) {
        egui::Window::new("Debug info").show(context.egui_context, |ui| {
            let color = match dt.as_millis() {
                0..=25 => [51, 204, 51],
                26..=50 => [255, 153, 0],
                _ => [204, 51, 51],
            };
            ui.colored_label(
                egui::Color32::from_rgb(color[0], color[1], color[2]),
                format!("FPS: {} ({}ms)", 1.0 / dt.as_secs_f32(), dt.as_millis()),
            );
        });
    }

    fn on_drop(&mut self, context: &mut morrigu::application::StateContext) {
        self.scene.destroy(context.renderer);
    }
}
