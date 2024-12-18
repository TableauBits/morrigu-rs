mod components;
mod ecs_buffer;
mod systems;

use crate::utils::{startup_state::SwitchableStates, ui::draw_debug_utils};

use super::utils::camera::MachaCamera;
use bevy_ecs::prelude::Entity;
use components::{
    macha_options::{MachaEntityOptions, MachaGlobalOptions},
    selected_entity::SelectedEntity,
};
use ecs_buffer::ECSBuffer;
use morrigu::{
    allocated_types::AllocatedBuffer,
    application::{
        event::WindowEvent, ApplicationState, BuildableApplicationState, EguiUpdateContext,
        StateContext,
    },
    bevy_ecs,
    components::{
        camera::{Camera, PerspectiveData},
        mesh_rendering,
        resource_wrapper::ResourceWrapper,
        transform::Transform,
    },
    descriptor_resources::DescriptorResources,
    egui,
    math_types::Vec2,
    shader::Shader,
    systems::mesh_renderer,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
    winit,
};
use systems::hierarchy_panel;
use transform_gizmo::{EnumSet, GizmoMode};
use winit::{event::KeyEvent, keyboard::KeyCode};

use std::path::Path;

use self::systems::gizmo_drawer;

type Vertex = morrigu::vertices::textured::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = mesh_rendering::MeshRendering<Vertex>;

pub struct MachaState {
    camera: MachaCamera,

    shader_ref: ThreadSafeRef<Shader>,
    material_ref: ThreadSafeRef<Material>,
    mesh_ref: ThreadSafeRef<Mesh>,
    mesh_rendering_ref: ThreadSafeRef<MeshRendering>,
    texture_ref: ThreadSafeRef<Texture>,
    flowmap_ref: ThreadSafeRef<Texture>,
    gradient_ref: ThreadSafeRef<Texture>,
    egui_texture_id: egui::TextureId,

    shader_options: Vec2,
    desired_state: SwitchableStates,
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
        let mut camera = MachaCamera::new(camera);
        camera.move_speed = 4.0;
        camera.set_distance(7.0);

        let shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/test/test.vert"),
            include_bytes!("shaders/gen/test/test.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create shader");

        let material_ref = Material::builder()
            .build(&shader_ref, DescriptorResources::empty(), context.renderer)
            .expect("Failed to create material");

        let mesh_ref = Vertex::load_model_from_path_obj(
            Path::new("assets/meshes/sphere.obj"),
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
                                .with_name("Shader options")
                                .build_with_pod(shader_options, context.renderer)
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
            desired_state: SwitchableStates::Editor,
        }
    }
}

impl ApplicationState for MachaState {
    fn on_attach(&mut self, context: &mut StateContext) {
        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_systems(mesh_renderer::render_meshes::<Vertex>);
        });

        context
            .ecs_manager
            .redefine_ui_systems_schedule(|schedule| {
                schedule.add_systems(hierarchy_panel::draw_hierarchy_panel_stable);
                schedule.add_systems(gizmo_drawer::draw_gizmo);
            });

        let res = context.renderer.window_resolution();
        self.camera.on_resize(res.0, res.1);

        let transform = Transform::default();
        self.camera.set_focal_point(transform.translation());

        context.ecs_manager.world.insert_resource(ECSBuffer::new());
        context
            .ecs_manager
            .world
            .insert_resource(MachaGlobalOptions::new());

        context.ecs_manager.world.spawn((
            transform.clone(),
            self.mesh_rendering_ref.clone(),
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

        let selection_style = egui::style::Selection {
            bg_fill: egui::Color32::from_rgb(165, 20, 61),
            ..Default::default()
        };
        let mut new_visuals = egui::Visuals::dark();
        new_visuals.selection = selection_style;

        context
            .egui
            .egui_platform_state
            .egui_ctx()
            .set_visuals(new_visuals);

        let egui_texture = Texture::builder()
            .build_from_path(
                Path::new("assets/textures/jupiter_base.png"),
                context.renderer,
            )
            .expect("Failed to build egui texture");
        self.egui_texture_id = context.egui.painter.register_user_texture(egui_texture);
    }

    fn on_drop(&mut self, context: &mut StateContext) {
        if let Some(texture) = context
            .egui
            .painter
            .retrieve_user_texture(self.egui_texture_id)
        {
            texture.lock().destroy(context.renderer);
        }

        self.mesh_rendering_ref
            .lock()
            .descriptor_resources
            .uniform_buffers
            .get(&0)
            .unwrap()
            .lock()
            .destroy(&context.renderer.device, &mut context.renderer.allocator());

        self.mesh_rendering_ref
            .lock()
            .descriptor_resources
            .uniform_buffers
            .get(&4)
            .unwrap()
            .lock()
            .destroy(&context.renderer.device, &mut context.renderer.allocator());

        self.gradient_ref.lock().destroy(context.renderer);
        self.flowmap_ref.lock().destroy(context.renderer);
        self.texture_ref.lock().destroy(context.renderer);
        self.mesh_rendering_ref.lock().destroy(context.renderer);
        self.mesh_ref.lock().destroy(context.renderer);
        self.material_ref.lock().destroy(context.renderer);
        self.shader_ref.lock().destroy(&context.renderer.device);
    }

    fn on_update(&mut self, dt: std::time::Duration, context: &mut StateContext) {
        // https://github.com/urholaukkarinen/egui-gizmo/issues/29
        if !context.window_input_state.held_alt() {
            self.camera.on_update(dt, context.window_input_state);
        }

        context
            .ecs_manager
            .world
            .insert_resource(self.camera.mrg_camera);
        context
            .ecs_manager
            .world
            .insert_resource(ResourceWrapper::new(context.window_input_state.clone()));
    }

    fn on_update_egui(&mut self, dt: std::time::Duration, context: &mut EguiUpdateContext) {
        draw_debug_utils(context.egui_context, dt, &mut self.desired_state);

        egui::Window::new("Shader uniforms").show(context.egui_context, |ui| {
            let image = egui::ImageSource::Texture(
                (self.egui_texture_id, egui::Vec2::new(128.0, 128.0)).into(),
            );
            ui.image(image);
            ui.add(egui::Slider::new(&mut self.shader_options[0], 0.0..=1.0).text("flow speed"));
            ui.add(
                egui::Slider::new(&mut self.shader_options[1], 0.0..=1.0).text("flow intensity"),
            );

            if ui.button("Apply changes").clicked() {
                self.mesh_rendering_ref
                    .lock()
                    .update_uniform_pod(4, self.shader_options)
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
                        .iter(&context.ecs_manager.world)
                        .for_each(|(entity, _)| {
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

    fn on_window_event(&mut self, event: WindowEvent, context: &mut StateContext) {
        self.camera.on_event(&event);

        if context.window_input_state.held_alt() {
            #[allow(clippy::single_match)] // Temporary
            match event {
                WindowEvent::KeyboardInput { event, .. } => self.on_keyboard_input(event, context),
                _ => (),
            }
        }
    }

    fn flow<'flow>(
        &mut self,
        context: &mut StateContext,
    ) -> morrigu::application::StateFlow<'flow> {
        match self.desired_state {
            SwitchableStates::GLTFLoader => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::gltf_loader::GLTFViewerState::build(context, ()),
            )),
            SwitchableStates::CSTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::compute_shader_test::CSTState::build(context, ()),
            )),
            SwitchableStates::PBRTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::pbr_test::PBRState::build(context, ()),
            )),

            #[cfg(feature = "ray_tracing")]
            SwitchableStates::RTTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::rt_test::RayTracerState::build(context, ()),
            )),
            SwitchableStates::Editor => morrigu::application::StateFlow::Continue,
        }
    }
}

fn set_gizmo(context: &mut StateContext, new_gizmo: EnumSet<GizmoMode>) {
    let gizmo = &mut context
        .ecs_manager
        .world
        .get_resource_mut::<MachaGlobalOptions>()
        .unwrap()
        .gizmo;

    let mut config = *gizmo.config();
    config.modes = new_gizmo;
    gizmo.update_config(config);
}

impl MachaState {
    fn on_keyboard_input(&mut self, input: KeyEvent, context: &mut StateContext) {
        if let winit::keyboard::PhysicalKey::Code(keycode) = input.physical_key {
            match keycode {
                KeyCode::KeyQ => set_gizmo(context, GizmoMode::all_translate()),
                KeyCode::KeyE => set_gizmo(context, GizmoMode::all_rotate()),
                KeyCode::KeyR => set_gizmo(context, GizmoMode::all_scale()),

                _ => (),
            }
        }
    }
}
