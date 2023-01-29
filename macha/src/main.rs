mod camera;
mod components;
mod ecs_buffer;
mod systems;

use bevy_ecs::{prelude::Entity, schedule::SystemStage};
use camera::MachaEditorCamera;
use components::{
    macha_options::{MachaEntityOptions, MachaGlobalOptions},
    selected_entity::SelectedEntity,
};
use ecs_buffer::ECSBuffer;
use morrigu::{
    allocated_types::AllocatedBuffer,
    application::{
        ApplicationBuilder, ApplicationState, BuildableApplicationState, EguiUpdateContext, Event,
        StateContext,
    },
    components::{
        camera::{Camera, PerspectiveData},
        mesh_rendering,
        resource_wrapper::ResourceWrapper,
        transform::{Axis, Transform},
    },
    compute_shader::ComputeShader,
    descriptor_resources::DescriptorResources,
    shader::Shader,
    systems::mesh_renderer,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
    vector_type::Vec2,
};
use systems::{gizmo_drawer, hierarchy_panel};
use winit::event::{KeyboardInput, VirtualKeyCode};

use std::path::Path;

type Vertex = morrigu::sample_vertex::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

struct MachaState {
    camera: MachaEditorCamera,

    shader_ref: ThreadSafeRef<Shader>,
    material_ref: ThreadSafeRef<Material>,
    mesh_ref: ThreadSafeRef<Mesh>,
    mesh_rendering_ref: ThreadSafeRef<MeshRendering>,
    texture_ref: ThreadSafeRef<Texture>,
    flowmap_ref: ThreadSafeRef<Texture>,
    gradient_ref: ThreadSafeRef<Texture>,
    egui_texture_id: egui::TextureId,

    shader_options: Vec2,
}

impl BuildableApplicationState<()> for MachaState {
    fn build(context: &mut StateContext, _: ()) -> Self {
        let flow_speed = 0.2_f32;
        let flow_intensity = 0.4_f32;
        let shader_options = Vec2::new(flow_speed, flow_intensity);

        let camera = Camera::builder().build(
            morrigu::components::camera::Projection::Perspective(PerspectiveData {
                horizontal_fov: f32::to_radians(50.0),
                near_plane: 0.001,
                far_plane: 1000.0,
            }),
            &Vec2::new(1280.0, 720.0),
        );
        let mut camera = MachaEditorCamera::new(camera);

        let shader_ref = Shader::from_spirv_u8(
            include_bytes!("../assets/gen/shaders/test/test.vert"),
            include_bytes!("../assets/gen/shaders/test/test.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create shader");

        let material_ref = Material::builder()
            .build(&shader_ref, DescriptorResources::empty(), context.renderer)
            .expect("Failed to create material");

        let mesh_ref = Vertex::load_model_from_path_ply(
            Path::new("assets/meshes/sphere.ply"),
            context.renderer,
        )
        .expect("Failed to create mesh");

        let texture_ref = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .build_from_path(
                Path::new("assets/textures/jupiter_base.png"),
                context.renderer,
            )
            .expect("Failed to load texture");
        let flowmap_ref = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .build_from_path(
                Path::new("assets/textures/jupiter_flowmap.png"),
                context.renderer,
            )
            .expect("Failed to load flowmap texture");
        let gradient_ref = Texture::builder()
            .build_from_path(
                Path::new("assets/textures/jupiter_gradient.png"),
                context.renderer,
            )
            .expect("Failed to load gradient texture");

        let shader_options_size: u64 = std::mem::size_of::<Vec2>().try_into().unwrap();
        let mesh_rendering_ref = MeshRendering::new(
            &mesh_ref,
            &material_ref,
            DescriptorResources {
                uniform_buffers: [
                    mesh_rendering::default_ubo_bindings(context.renderer).unwrap(),
                    (
                        4,
                        ThreadSafeRef::new(
                            AllocatedBuffer::builder(shader_options_size)
                                .build(context.renderer)
                                .unwrap(),
                        ),
                    ),
                ]
                .into(),
                sampled_images: [
                    (1, texture_ref.clone()),
                    (2, flowmap_ref.clone()),
                    (3, gradient_ref.clone()),
                ]
                .into(),
                ..Default::default()
            },
            context.renderer,
        )
        .expect("Failed to create mesh rendering");

        let mut transform = Transform::default();
        transform.rotate(f32::to_radians(-90.0), Axis::X);

        camera.set_focal_point(transform.position());

        context.ecs_manager.world.insert_resource(ECSBuffer::new());
        context
            .ecs_manager
            .world
            .insert_resource(MachaGlobalOptions::new());

        context.ecs_manager.world.spawn((
            transform,
            mesh_rendering_ref.clone(),
            MachaEntityOptions {
                name: "planet".to_owned(),
            },
        ));

        context.ecs_manager.world.spawn((
            transform,
            MachaEntityOptions {
                name: "empty".to_owned(),
            },
        ));

        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_stage(
                "render meshes",
                SystemStage::parallel().with_system(mesh_renderer::render_meshes::<Vertex>),
            );
        });

        context
            .ecs_manager
            .redefine_ui_systems_schedule(|schedule| {
                schedule.add_stage(
                    "hierarchy panel",
                    SystemStage::parallel()
                        .with_system(hierarchy_panel::draw_hierarchy_panel_stable),
                );
                schedule.add_stage(
                    "gizmo",
                    SystemStage::parallel().with_system(gizmo_drawer::draw_gizmo),
                );
            });

        /*
        let input_texture = texture_ref.lock();
        let output_compute_texture = Texture::builder()
            .build_from_data(
                &std::iter::repeat(u8::MAX)
                    .take(
                        (input_texture.dimensions[0] * input_texture.dimensions[1] * 4)
                            .try_into()
                            .unwrap(),
                    )
                    .collect::<Vec<_>>(),
                input_texture.dimensions[0],
                input_texture.dimensions[1],
                context.renderer,
            )
            .unwrap();
        drop(input_texture);
        let compute_shader = ComputeShader::builder()
            .build_from_spirv_u8(
                include_bytes!("../assets/gen/shaders/test/test.comp"),
                DescriptorResources {
                    storage_images: [
                        (0, texture_ref.lock().image_ref.clone()),
                        (1, output_compute_texture.lock().image_ref.clone()),
                    ]
                    .into(),
                    ..Default::default()
                },
                context.renderer,
            )
            .expect("Failed to create compute shader");
        */

        MachaState {
            camera,
            shader_ref,
            material_ref,
            mesh_ref,
            mesh_rendering_ref,
            texture_ref,
            flowmap_ref,
            gradient_ref,
            egui_texture_id: egui::TextureId::default(),
            shader_options,
        }
    }
}

impl ApplicationState for MachaState {
    fn on_attach(&mut self, context: &mut StateContext) {
        let selection_style = egui::style::Selection {
            bg_fill: egui::Color32::from_rgb(165, 20, 61),
            ..Default::default()
        };
        let mut new_visuals = egui::Visuals::dark();
        new_visuals.selection = selection_style;

        context.egui.context.set_visuals(new_visuals);

        let egui_texture = Texture::builder()
            .build_from_path(
                std::path::Path::new("assets/textures/jupiter_base.png"),
                context.renderer,
            )
            .expect("Failed to build egui texture");
        self.egui_texture_id = context.egui.painter.register_user_texture(egui_texture);
    }

    fn on_update(&mut self, dt: std::time::Duration, context: &mut StateContext) {
        self.camera.on_update(dt, context.window_input_state);
        context
            .ecs_manager
            .world
            .insert_resource(ResourceWrapper::new(context.window_input_state.clone()));
        context
            .ecs_manager
            .world
            .insert_resource(self.camera.mrg_camera);
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
        egui::Window::new("Shader uniforms").show(context.egui_context, |ui| {
            ui.image(self.egui_texture_id, (128.0, 128.0));
            ui.add(egui::Slider::new(&mut self.shader_options[0], 0.0..=1.0).text("flow speed"));
            ui.add(
                egui::Slider::new(&mut self.shader_options[1], 0.0..=1.0).text("flow intensity"),
            );

            if ui.button("Apply changes").clicked() {
                self.mesh_rendering_ref
                    .lock()
                    .update_uniform(4, self.shader_options)
                    .expect("Failed to upload flow settings");
            }
        });
    }

    fn after_ui_systems(&mut self, _dt: std::time::Duration, context: &mut EguiUpdateContext) {
        // Re-take ownership of buffer while processing it
        let mut ecs_buffer = context
            .ecs_manager
            .world
            .remove_resource::<ECSBuffer>()
            .expect("Failed to fetch ECS command buffer");
        for job in &ecs_buffer.command_buffer {
            match job {
                ecs_buffer::ECSJob::SelectEntity {
                    entity: new_selected_entity,
                } => {
                    let mut old_selected = None;
                    context
                        .ecs_manager
                        .world
                        .query::<(Entity, &SelectedEntity)>()
                        .for_each(&context.ecs_manager.world, |(entity, _)| {
                            old_selected = Some(entity);
                        });
                    if let Some(old_selected_entity) = old_selected {
                        context
                            .ecs_manager
                            .world
                            .entity_mut(old_selected_entity)
                            .remove::<SelectedEntity>();
                    }
                    if let Some(new_selected_entity) = new_selected_entity {
                        context
                            .ecs_manager
                            .world
                            .entity_mut(*new_selected_entity)
                            .insert(SelectedEntity {});
                    }
                }
            }
        }
        ecs_buffer.command_buffer.clear();
        context.ecs_manager.world.insert_resource(ecs_buffer);
    }

    fn on_event(&mut self, event: Event<()>, context: &mut StateContext) {
        #[allow(clippy::single_match)] // Temporary
        match event {
            morrigu::application::Event::WindowEvent {
                event: winit::event::WindowEvent::KeyboardInput { input, .. },
                ..
            } => self.on_keyboard_input(input, context),
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

    fn on_drop(&mut self, context: &mut StateContext) {
        if let Some(texture) = context
            .egui
            .painter
            .retreive_user_texture(self.egui_texture_id)
        {
            texture.lock().destroy(context.renderer);
        }

        self.gradient_ref.lock().destroy(context.renderer);
        self.flowmap_ref.lock().destroy(context.renderer);
        self.texture_ref.lock().destroy(context.renderer);
        self.mesh_rendering_ref.lock().destroy(context.renderer);
        self.mesh_ref.lock().destroy(context.renderer);
        self.material_ref.lock().destroy(context.renderer);
        self.shader_ref.lock().destroy(&context.renderer.device);
    }
}

impl MachaState {
    fn on_keyboard_input(&mut self, input: KeyboardInput, context: &mut StateContext) {
        if input.virtual_keycode.is_none() {
            return;
        }
        let virtual_keycode = input.virtual_keycode.unwrap();

        match virtual_keycode {
            VirtualKeyCode::Q => {
                context
                    .ecs_manager
                    .world
                    .get_resource_mut::<MachaGlobalOptions>()
                    .unwrap()
                    .preferred_gizmo = egui_gizmo::GizmoMode::Translate
            }
            VirtualKeyCode::E => {
                context
                    .ecs_manager
                    .world
                    .get_resource_mut::<MachaGlobalOptions>()
                    .unwrap()
                    .preferred_gizmo = egui_gizmo::GizmoMode::Rotate
            }
            VirtualKeyCode::R => {
                context
                    .ecs_manager
                    .world
                    .get_resource_mut::<MachaGlobalOptions>()
                    .unwrap()
                    .preferred_gizmo = egui_gizmo::GizmoMode::Scale
            }

            _ => (),
        }
    }
}

fn init_logging() {
    #[cfg(debug_assertions)]
    let log_level = ("trace", flexi_logger::Duplicate::Debug);
    #[cfg(not(debug_assertions))]
    let log_level = ("info", flexi_logger::Duplicate::Info);

    let file_spec = flexi_logger::FileSpec::default().suppress_timestamp();

    let _logger = flexi_logger::Logger::try_with_env_or_str(log_level.0)
        .expect("Failed to setup logging")
        .log_to_file(file_spec)
        .write_mode(flexi_logger::WriteMode::BufferAndFlush)
        .duplicate_to_stdout(log_level.1)
        .set_palette("b9;3;2;8;7".to_owned())
        .start()
        .expect("Failed to build logger");
}

fn main() {
    init_logging();

    ApplicationBuilder::new()
        .with_window_name("Macha editor")
        .with_dimensions(1280, 720)
        .with_application_name("Macha")
        .with_application_version(0, 1, 0)
        .build_and_run_inplace::<MachaState, ()>(());
}
