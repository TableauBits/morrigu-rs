use std::path::Path;

use morrigu::ash::vk;
use morrigu::material::CullModeFlags;
use morrigu::{
    application::{ApplicationState, BuildableApplicationState, EguiUpdateContext},
    components::{
        camera::{Camera, PerspectiveData},
        mesh_rendering,
        transform::Transform,
    },
    compute_shader::ComputeShader,
    descriptor_resources::DescriptorResources,
    math_types::{EulerRot, Quat, Vec2, Vec3},
    pipeline_barrier::PipelineBarrier,
    shader::Shader,
    systems::mesh_renderer,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
};

use crate::utils::startup_state::SwitchableStates;
use crate::utils::ui::draw_debug_utils;

type Vertex = morrigu::vertices::textured::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type MeshRendering = mesh_rendering::MeshRendering<Vertex>;

pub struct CSTState {
    camera: Camera,
    input_texture: ThreadSafeRef<Texture>,
    output_texture: ThreadSafeRef<Texture>,

    material_ref: ThreadSafeRef<Material>,
    input_mesh_rendering_ref: ThreadSafeRef<MeshRendering>,
    output_mesh_rendering_ref: ThreadSafeRef<MeshRendering>,

    desired_state: SwitchableStates,
}

impl BuildableApplicationState<()> for CSTState {
    fn build(context: &mut morrigu::application::StateContext, _: ()) -> Self {
        let camera = Camera::builder().build(
            morrigu::components::camera::Projection::Perspective(PerspectiveData {
                horizontal_fov: f32::to_radians(50.0),
                near_plane: 0.001,
                far_plane: 1000.0,
            }),
            &Vec2::new(1280.0, 720.0),
        );

        let input_texture = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .with_layout(vk::ImageLayout::GENERAL)
            .with_usage(vk::ImageUsageFlags::STORAGE)
            .build_from_path(
                Path::new("assets/textures/jupiter_base.png"),
                context.renderer,
            )
            .expect("Failed to load texture");
        let output_texture = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .with_layout(vk::ImageLayout::GENERAL)
            .with_usage(vk::ImageUsageFlags::STORAGE)
            .build(input_texture.lock().dimensions, context.renderer)
            .expect("Failed to load texture");

        let shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/textured/textured.vert"),
            include_bytes!("shaders/gen/textured/textured.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create shader");

        let material_ref = Material::builder()
            .cull_mode(CullModeFlags::NONE)
            .build::<Vertex>(&shader_ref, DescriptorResources::empty(), context.renderer)
            .expect("Failed to create material");

        let mesh_ref = Vertex::load_model_from_path_obj(
            Path::new("assets/meshes/plane.obj"),
            context.renderer,
        )
        .expect("Failed to create mesh");

        let input_mesh_rendering_ref = MeshRendering::new(
            &mesh_ref,
            &material_ref,
            DescriptorResources {
                uniform_buffers: [mesh_rendering::default_ubo_bindings(context.renderer).unwrap()]
                    .into(),
                sampled_images: [(1, input_texture.clone())].into(),
                ..Default::default()
            },
            context.renderer,
        )
        .expect("Failed to create mesh rendering");

        let output_mesh_rendering_ref = MeshRendering::new(
            &mesh_ref,
            &material_ref,
            DescriptorResources {
                uniform_buffers: [mesh_rendering::default_ubo_bindings(context.renderer).unwrap()]
                    .into(),
                sampled_images: [(1, output_texture.clone())].into(),
                ..Default::default()
            },
            context.renderer,
        )
        .expect("Failed to create mesh rendering");

        Self {
            camera,
            input_texture,
            output_texture,
            material_ref,
            output_mesh_rendering_ref,
            input_mesh_rendering_ref,
            desired_state: SwitchableStates::CSTest,
        }
    }
}

impl ApplicationState for CSTState {
    fn on_attach(&mut self, context: &mut morrigu::application::StateContext) {
        context.ecs_manager.redefine_systems_schedule(|schedule| {
            schedule.add_systems(mesh_renderer::render_meshes::<Vertex>);
        });

        let res = context.renderer.window_resolution();
        self.camera.on_resize(res.0, res.1);
        context.ecs_manager.world.insert_resource(self.camera);

        let mut transform = Transform::default();
        transform.rotate(&Quat::from_euler(
            EulerRot::XYZ,
            f32::to_radians(-90.0),
            0.0,
            0.0,
        ));
        transform.set_translation(&Vec3::new(-0.5, 0.0, -1.0));
        transform.rescale(&Vec3::new(0.3, 0.3, 0.3));
        context
            .ecs_manager
            .world
            .spawn((transform.clone(), self.input_mesh_rendering_ref.clone()));

        transform.set_translation(&Vec3::new(0.5, 0.0, -1.0));
        context
            .ecs_manager
            .world
            .spawn((transform, self.output_mesh_rendering_ref.clone()));

        let compute_shader = ComputeShader::builder()
            .build_from_spirv_u8(
                include_bytes!("shaders/gen/blur.comp"),
                DescriptorResources {
                    storage_images: [
                        (0, self.input_texture.lock().image_ref.clone()),
                        (1, self.output_texture.lock().image_ref.clone()),
                    ]
                    .into(),
                    ..Default::default()
                },
                context.renderer,
            )
            .expect("Failed to build compute shader");

        let [width, height] = self.input_texture.lock().dimensions;

        compute_shader
            .lock()
            .run(
                (width / 16, height / 16, 1),
                PipelineBarrier {
                    src_stage_mask: vk::PipelineStageFlags::COMPUTE_SHADER,
                    dst_stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
                    dependency_flags: vk::DependencyFlags::empty(),
                    memory_barriers: vec![],
                    buffer_memory_barriers: vec![],
                    image_memory_barriers: vec![
                        vk::ImageMemoryBarrier::default()
                            .old_layout(vk::ImageLayout::GENERAL)
                            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                            .image(self.input_texture.lock().image_ref.lock().handle)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            })
                            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                            .dst_access_mask(vk::AccessFlags::SHADER_READ),
                        vk::ImageMemoryBarrier::default()
                            .old_layout(vk::ImageLayout::GENERAL)
                            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                            .image(self.output_texture.lock().image_ref.lock().handle)
                            .subresource_range(vk::ImageSubresourceRange {
                                aspect_mask: vk::ImageAspectFlags::COLOR,
                                base_mip_level: 0,
                                level_count: 1,
                                base_array_layer: 0,
                                layer_count: 1,
                            })
                            .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                            .dst_access_mask(vk::AccessFlags::SHADER_READ),
                    ],
                },
                context.renderer,
            )
            .expect("Failed to run compute shader");

        compute_shader.lock().destroy(context.renderer);
    }

    fn on_drop(&mut self, context: &mut morrigu::application::StateContext) {
        self.output_mesh_rendering_ref
            .lock()
            .descriptor_resources
            .uniform_buffers[&0]
            .lock()
            .destroy(&context.renderer.device, &mut context.renderer.allocator());
        self.output_mesh_rendering_ref
            .lock()
            .destroy(context.renderer);
        self.input_mesh_rendering_ref
            .lock()
            .descriptor_resources
            .uniform_buffers[&0]
            .lock()
            .destroy(&context.renderer.device, &mut context.renderer.allocator());
        self.input_mesh_rendering_ref
            .lock()
            .destroy(context.renderer);

        self.output_mesh_rendering_ref
            .lock()
            .mesh_ref
            .lock()
            .destroy(context.renderer);
        self.material_ref.lock().destroy(context.renderer);
        self.material_ref
            .lock()
            .shader_ref
            .lock()
            .destroy(&context.renderer.device);

        self.output_texture.lock().destroy(context.renderer);
        self.input_texture.lock().destroy(context.renderer);
    }

    fn on_update_egui(&mut self, dt: std::time::Duration, context: &mut EguiUpdateContext) {
        draw_debug_utils(context.egui_context, dt, &mut self.desired_state);
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
            SwitchableStates::PBRTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::pbr_test::PBRState::build(context, ()),
            )),

            #[cfg(feature = "ray_tracing")]
            SwitchableStates::RTTest => morrigu::application::StateFlow::SwitchState(Box::new(
                crate::rt_test::RayTracerState::build(context, ()),
            )),
            SwitchableStates::CSTest => morrigu::application::StateFlow::Continue,
        }
    }
}
