mod camera;
mod loader;
mod scene;

use std::{iter::zip, path::Path};

use morrigu::{
    application::{ApplicationState, BuildableApplicationState, EguiUpdateContext, Event},
    components::{
        camera::{Camera, PerspectiveData},
        mesh_rendering::default_descriptor_resources,
        transform::Transform,
    },
    cubemap::Cubemap,
    descriptor_resources::DescriptorResources,
    math_types::{Quat, Vec2, Vec3, Vec4},
    shader::Shader,
    systems::mesh_renderer,
    utils::ThreadSafeRef,
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
    skybox_entity_ref: bevy_ecs::entity::Entity,
    skybox: ThreadSafeRef<SkyboxMeshRendering>,
}

type SkyboxVertex = morrigu::vertices::simple::SimpleVertex;
type SkyboxMaterial = morrigu::material::Material<SkyboxVertex>;
type SkyboxMeshRendering = morrigu::components::mesh_rendering::MeshRendering<SkyboxVertex>;

#[profiling::all_functions]
impl BuildableApplicationState<()> for GLTFViewerState {
    fn build(context: &mut morrigu::application::StateContext, _: ()) -> Self {
        let camera = Camera::builder().build(
            morrigu::components::camera::Projection::Perspective(PerspectiveData {
                horizontal_fov: f32::to_radians(50.0),
                near_plane: 0.001,
                far_plane: 1000.0,
            }),
            &Vec2::new(1280.0, 720.0),
        );

        let mut camera = ViewerCamera::new(camera);
        camera.set_distance(0.0);

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

        let skybox_cubemap = Cubemap::build_from_folder(
            "assets/textures/skybox",
            "jpg",
            morrigu::texture::TextureFormat::RGBA8_UNORM,
            context.renderer,
        )
        .expect("Failed to build skybox cubemap texture");
        let skybox_shader = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/cubemap/cubemap.vert"),
            include_bytes!("shaders/gen/cubemap/cubemap.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create skybox shader");
        let skybox_material: ThreadSafeRef<SkyboxMaterial> = Material::builder()
            .z_write(false)
            .build(
                &skybox_shader,
                DescriptorResources {
                    cubemap_images: [(0, skybox_cubemap)].into(),
                    ..Default::default()
                },
                context.renderer,
            )
            .expect("Failed to create skybox material");
        let skybox_mesh = SkyboxVertex::load_model_from_path_obj(
            Path::new("assets/meshes/cube.obj"),
            context.renderer,
        )
        .expect("Failed to load cube obj");
        let skybox = SkyboxMeshRendering::new(
            &skybox_mesh,
            &skybox_material,
            default_descriptor_resources(context.renderer)
                .expect("Failed to create default descriptor resources"),
            context.renderer,
        )
        .expect("Failed to create skybox mesh rendering");

        let scene = loader::load_gltf(
            Path::new("assets/scenes/sponza/Sponza.gltf"),
            // Transform::default(),
            Transform::from_trs(
                &Vec3::default(),
                &Quat::default(),
                &Vec3::new(10.0, 10.0, 10.0),
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

        let skybox_entity_ref = context
            .ecs_manager
            .world
            .spawn((
                Transform::from_trs(
                    camera.mrg_camera.position(),
                    &Quat::default(),
                    &Vec3::new(1.0, 1.0, 1.0),
                ),
                skybox.clone(),
            ))
            .id();

        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_systems(mesh_renderer::render_meshes::<Vertex>);
            schedule.add_systems(mesh_renderer::render_meshes::<SkyboxVertex>);
        });

        let light_data = LightData {
            light_direction: Vec4::new(-1.0, -1.0, 0.0, 0.0).normalize(),
            light_color: Vec4::new(0.68, 0.68, 0.68, 1.0),
            ambient_light_color: Vec3::new(0.3, 0.3, 0.3),
            ambient_light_intensity: 0.2,
            camera_position: *camera.mrg_camera.position(),
            __padding: 0.0,
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
            skybox_entity_ref,
            skybox,
        }
    }
}

#[profiling::all_functions]
impl ApplicationState for GLTFViewerState {
    fn on_attach(&mut self, _context: &mut morrigu::application::StateContext) {}

    fn on_update(
        &mut self,
        dt: std::time::Duration,
        context: &mut morrigu::application::StateContext,
    ) {
        self.camera.on_update(dt, context.window_input_state);

        let cam_pos = self.camera.mrg_camera.position();
        self.light_data.camera_position = *cam_pos;

        let mut entity_ref = context
            .ecs_manager
            .world
            .get_entity_mut(self.skybox_entity_ref)
            .expect("Failed to retreive skybox entity");
        if let Some(mut transform) = entity_ref.get_mut::<Transform>() {
            transform.set_translation(cam_pos);
        }

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

    fn on_update_egui(&mut self, _dt: std::time::Duration, context: &mut EguiUpdateContext) {
        puffin_egui::profiler_window(context.egui_context);
    }

    fn on_drop(&mut self, context: &mut morrigu::application::StateContext) {
        let mut skybox = self.skybox.lock();
        skybox.destroy(context.renderer);
        skybox
            .descriptor_resources
            .uniform_buffers
            .values()
            .for_each(|buffer| {
                buffer
                    .lock()
                    .destroy(&context.renderer.device, &mut context.renderer.allocator())
            });
        let mut skybox_material = skybox.material_ref.lock();
        skybox_material.destroy(context.renderer);
        skybox_material
            .descriptor_resources
            .cubemap_images
            .values()
            .for_each(|image| image.lock().destroy(context.renderer));
        skybox_material
            .shader_ref
            .lock()
            .destroy(&context.renderer.device);
        skybox.mesh_ref.lock().destroy(context.renderer);

        self.scene.destroy(context.renderer);
    }
}
