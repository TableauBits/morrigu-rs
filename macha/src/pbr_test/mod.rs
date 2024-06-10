use std::{mem::size_of, path::Path};

use morrigu::{
    allocated_types::AllocatedBuffer,
    application::{ApplicationState, BuildableApplicationState},
    bevy_ecs::entity::Entity,
    components::transform::Transform,
    descriptor_resources::DescriptorResources,
    egui::{self},
    glam::vec3,
    math_types::{Vec2, Vec3, Vec4},
    shader::Shader,
    utils::ThreadSafeRef,
};

use crate::utils::{camera::MachaCamera, startup_state::SwitchableStates, ui::draw_debug_utils};

type Vertex = morrigu::vertices::textured::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type Mesh = morrigu::mesh::Mesh<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct LightData {
    camera_pos: Vec4,
    light_pos: Vec4,
}
unsafe impl bytemuck::Zeroable for LightData {}
unsafe impl bytemuck::Pod for LightData {}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct PBRData {
    albedo: Vec4,
    mra: Vec4,
}
unsafe impl bytemuck::Zeroable for PBRData {}
unsafe impl bytemuck::Pod for PBRData {}

pub struct PBRState {
    camera: MachaCamera,
    camera_focus: Option<usize>,

    point_light_angle: f32,
    point_light_debug: ThreadSafeRef<MeshRendering>,
    point_light_entity: Entity,

    flat_shader_ref: ThreadSafeRef<Shader>,
    pbr_shader_ref: ThreadSafeRef<Shader>,

    flat_material_ref: ThreadSafeRef<Material>,
    diffuse_material_ref: ThreadSafeRef<Material>,

    mesh_ref: ThreadSafeRef<Mesh>,
    mesh_renderings_ref: Vec<ThreadSafeRef<MeshRendering>>,
    entities: Vec<Entity>,

    desired_state: SwitchableStates,
}

impl BuildableApplicationState<()> for PBRState {
    fn build(context: &mut morrigu::application::StateContext, _data: ()) -> Self {
        let flat_shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/flat/flat.vert"),
            include_bytes!("shaders/gen/flat/flat.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create flat shader");
        let pbr_shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/pbr/pbr.vert"),
            include_bytes!("shaders/gen/pbr/pbr.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create pbr shader");

        let mesh_ref = Vertex::load_model_from_path_obj(
            Path::new("assets/meshes/sphere.obj"),
            context.renderer,
        )
        .expect("Failed to create mesh");

        let mut mesh_renderings = vec![];

        // sample flat material, for debug model
        let flat_material_ref = Material::builder()
            .build(
                &flat_shader_ref,
                DescriptorResources::empty(),
                context.renderer,
            )
            .expect("Failed to create material");

        let point_light_debug = MeshRendering::new(
            &mesh_ref,
            &flat_material_ref,
            DescriptorResources {
                uniform_buffers: [morrigu::components::mesh_rendering::default_ubo_bindings(
                    context.renderer,
                )
                .unwrap()]
                .into(),
                ..Default::default()
            },
            context.renderer,
        )
        .expect("Failed to create mesh rendering");
        point_light_debug.lock().visible = false;

        let pbr_material_ref = Material::builder()
            .build(
                &pbr_shader_ref,
                DescriptorResources {
                    uniform_buffers: [(
                        0,
                        ThreadSafeRef::new(
                            AllocatedBuffer::builder(size_of::<LightData>() as u64)
                                .with_name("Light data")
                                .build(context.renderer)
                                .expect("Failed to build light data buffer"),
                        ),
                    )]
                    .into(),
                    ..Default::default()
                },
                context.renderer,
            )
            .expect("Failed to create material");

        let grid_size = 7;
        for i in 0..grid_size {
            for j in 0..grid_size {
                let pbr_data = PBRData {
                    albedo: Vec4::new(0.8, 0.1, 0.1, 0.0),
                    mra: Vec4::new(
                        1.0 - ((1.0 / (grid_size - 1) as f32) * i as f32),
                        (1.0 / (grid_size - 1) as f32) * j as f32,
                        1.0,
                        0.0,
                    ),
                };

                let mesh_rendering_ref = MeshRendering::new(
                    &mesh_ref,
                    &pbr_material_ref,
                    DescriptorResources {
                        uniform_buffers: [
                            morrigu::components::mesh_rendering::default_ubo_bindings(
                                context.renderer,
                            )
                            .unwrap(),
                            (
                                1,
                                ThreadSafeRef::new(
                                    AllocatedBuffer::builder(size_of::<PBRData>() as u64)
                                        .with_name("PBR data")
                                        .build_with_pod(pbr_data, context.renderer)
                                        .expect("Failed to build light data buffer"),
                                ),
                            ),
                        ]
                        .into(),
                        ..Default::default()
                    },
                    context.renderer,
                )
                .expect("Failed to create mesh rendering");

                mesh_renderings.push(mesh_rendering_ref);
            }
        }

        let camera = morrigu::components::camera::Camera::builder().build(
            morrigu::components::camera::Projection::Perspective(
                morrigu::components::camera::PerspectiveData {
                    horizontal_fov: (60.0_f32).to_radians(),
                    near_plane: 0.001,
                    far_plane: 1000.0,
                },
            ),
            &Vec2::new(1280.0, 720.0),
        );

        Self {
            camera: MachaCamera::new(camera),
            camera_focus: None,

            point_light_angle: 0.0,
            point_light_debug,
            point_light_entity: Entity::PLACEHOLDER,

            flat_shader_ref,
            pbr_shader_ref,

            flat_material_ref,
            diffuse_material_ref: pbr_material_ref,

            mesh_ref,
            mesh_renderings_ref: mesh_renderings,
            entities: vec![],

            desired_state: SwitchableStates::PBRTest,
        }
    }
}

impl ApplicationState for PBRState {
    fn on_attach(&mut self, context: &mut morrigu::application::StateContext) {
        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_systems(morrigu::systems::mesh_renderer::render_meshes::<Vertex>);
        });

        let res = context.renderer.window_resolution();
        self.camera.on_resize(res.0, res.1);

        let mut dbg_transform = Transform::default();
        dbg_transform.set_scale(&vec3(0.4, 0.4, 0.4));
        dbg_transform.translate(&Vec3::new(0.0, 15.0, 0.0));
        self.point_light_entity = context
            .ecs_manager
            .world
            .spawn((dbg_transform, self.point_light_debug.clone()))
            .id();

        let transform = Transform::default();
        self.camera.set_focal_point(transform.translation());
        self.camera.set_distance(25.0);

        for (idx, mrr) in self.mesh_renderings_ref.iter().enumerate() {
            let grid_size = 7;
            let i = idx / grid_size;
            let j = idx % grid_size;

            let mut transform = transform.clone();
            transform.translate(&Vec3::new(
                ((20.0 / ((grid_size - 1) as f32)) * j as f32) - 10.0,
                ((20.0 / ((grid_size - 1) as f32)) * i as f32) - 10.0,
                0.0,
            ));

            let entity = context
                .ecs_manager
                .world
                .spawn((transform, mrr.clone()))
                .id();
            self.entities.push(entity);
        }
    }

    fn on_drop(&mut self, context: &mut morrigu::application::StateContext) {
        for mrr in &mut self.mesh_renderings_ref {
            mrr.lock()
                .descriptor_resources
                .uniform_buffers
                .get(&0)
                .unwrap()
                .lock()
                .destroy(&context.renderer.device, &mut context.renderer.allocator());
            if let Some(buffer) = mrr.lock().descriptor_resources.uniform_buffers.get(&1) {
                buffer
                    .lock()
                    .destroy(&context.renderer.device, &mut context.renderer.allocator());
            }
            mrr.lock().destroy(context.renderer);
        }
        self.point_light_debug
            .lock()
            .descriptor_resources
            .uniform_buffers
            .get(&0)
            .unwrap()
            .lock()
            .destroy(&context.renderer.device, &mut context.renderer.allocator());
        self.point_light_debug.lock().destroy(context.renderer);
        self.mesh_ref.lock().destroy(context.renderer);

        self.flat_material_ref.lock().destroy(context.renderer);

        self.diffuse_material_ref
            .lock()
            .descriptor_resources
            .uniform_buffers
            .get(&0)
            .unwrap()
            .lock()
            .destroy(&context.renderer.device, &mut context.renderer.allocator());
        self.diffuse_material_ref.lock().destroy(context.renderer);

        self.pbr_shader_ref.lock().destroy(&context.renderer.device);
        self.flat_shader_ref
            .lock()
            .destroy(&context.renderer.device);
    }

    fn on_update(
        &mut self,
        dt: std::time::Duration,
        context: &mut morrigu::application::StateContext,
    ) {
        self.camera.on_update(dt, context.window_input_state);
        context
            .ecs_manager
            .world
            .insert_resource(self.camera.mrg_camera);

        let light_pos = 17.0
            * Vec3::new(
                self.point_light_angle.to_radians().cos(),
                self.point_light_angle.to_radians().sin(),
                0.0,
            );
        let light_data = LightData {
            camera_pos: (*self.camera.mrg_camera.position(), 0.0).into(),
            light_pos: (light_pos, 0.0).into(),
        };

        self.diffuse_material_ref
            .lock()
            .update_uniform(0, light_data)
            .expect("Failed to update ligth data buffer");

        context
            .ecs_manager
            .world
            .get_entity_mut(self.point_light_entity)
            .unwrap()
            .get_mut::<Transform>()
            .unwrap()
            .set_translation(&light_pos);
    }

    fn on_update_egui(
        &mut self,
        dt: std::time::Duration,
        context: &mut morrigu::application::EguiUpdateContext,
    ) {
        draw_debug_utils(context.egui_context, dt, &mut self.desired_state);

        egui::Window::new("Light controls").show(context.egui_context, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::LEFT), |ui| {
                ui.label("Select camera focus");
                egui::ComboBox::from_id_source("Select camera focus")
                    .selected_text(match self.camera_focus {
                        Some(idx) => idx.to_string(),
                        None => "Whole scene".to_owned(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.camera_focus, None, "Whole scene");
                        for target_idx in 0..self.mesh_renderings_ref.len() {
                            ui.selectable_value(
                                &mut self.camera_focus,
                                Some(target_idx),
                                target_idx.to_string(),
                            );
                        }
                    });
                if ui.button("Apply camera focus").clicked() {
                    let (target_pos, distance) = match self.camera_focus {
                        Some(target_idx) => (
                            *context
                                .ecs_manager
                                .world
                                .get_entity(*self.entities.get(target_idx).unwrap())
                                .unwrap()
                                .get::<Transform>()
                                .unwrap()
                                .translation(),
                            7.0,
                        ),
                        None => (Vec3::default(), 25.0),
                    };
                    self.camera.set_focal_point(&target_pos);
                    self.camera.set_distance(distance);
                }
            });

            ui.add(
                egui::Slider::new(&mut self.point_light_angle, 0.0..=360.0)
                    .text("point light angle")
                    .smart_aim(false)
                    .step_by(0.1),
            );
            ui.checkbox(
                &mut context
                    .ecs_manager
                    .world
                    .get_entity_mut(self.point_light_entity)
                    .unwrap()
                    .get_mut::<ThreadSafeRef<MeshRendering>>()
                    .unwrap()
                    .lock()
                    .visible,
                "enable debug light view",
            )
        });
    }

    fn on_event(
        &mut self,
        event: morrigu::application::Event<()>,
        _context: &mut morrigu::application::StateContext,
    ) {
        self.camera.on_event(&event);
    }

    fn flow<'flow>(
        &mut self,
        context: &mut morrigu::application::StateContext,
    ) -> morrigu::application::StateFlow<'flow> {
        match self.desired_state {
            SwitchableStates::Editor => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::editor::MachaState::build(context, ()),
            )),
            SwitchableStates::GLTFLoader => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::gltf_loader::GLTFViewerState::build(context, ()),
            )),
            SwitchableStates::CSTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::compute_shader_test::CSTState::build(context, ()),
            )),

            #[cfg(feature = "ray_tracing")]
            SwitchableStates::RTTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::rt_test::RayTracerState::build(context, ()),
            )),
            SwitchableStates::PBRTest => morrigu::application::StateFlow::Continue,
        }
    }
}
