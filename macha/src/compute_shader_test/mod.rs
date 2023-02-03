use std::path::Path;

use ash::vk;
use morrigu::{
    application::{ApplicationState, BuildableApplicationState},
    components::{
        camera::{Camera, PerspectiveData},
        mesh_rendering,
        transform::Transform,
    },
    compute_shader::ComputeShader,
    descriptor_resources::DescriptorResources,
    pipeline_barrier::PipelineBarrier,
    shader::Shader,
    texture::{Texture, TextureFormat},
    utils::ThreadSafeRef,
    vector_type::Vec2,
};

type Vertex = morrigu::sample_vertex::TexturedVertex;
type Material = morrigu::material::Material<Vertex>;
type MeshRendering = morrigu::components::mesh_rendering::MeshRendering<Vertex>;

pub struct CSTState {
    input_texture: ThreadSafeRef<Texture>,
    output_texture: ThreadSafeRef<Texture>,

    material_ref: ThreadSafeRef<Material>,
    mesh_rendering_ref: ThreadSafeRef<MeshRendering>,
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
            .with_usage(vk::ImageUsageFlags::STORAGE)
            .build_from_path(
                Path::new("assets/textures/jupiter_base.png"),
                context.renderer,
            )
            .expect("Failed to load texture");
        let output_texture = Texture::builder()
            .with_format(TextureFormat::RGBA8_UNORM)
            .with_usage(vk::ImageUsageFlags::STORAGE)
            .build(context.renderer)
            .expect("Failed to load texture");

        let shader_ref = Shader::from_spirv_u8(
            include_bytes!("shaders/gen/textured/textured.vert"),
            include_bytes!("shaders/gen/textured/textured.frag"),
            &context.renderer.device,
        )
        .expect("Failed to create shader");

        let material_ref = Material::builder()
            .build::<Vertex>(&shader_ref, DescriptorResources::empty(), context.renderer)
            .expect("Failed to create material");

        let mesh_ref = Vertex::load_model_from_path_obj(
            Path::new("assets/meshes/plane.obj"),
            context.renderer,
        )
        .expect("Failed to create mesh");

        let mesh_rendering_ref = MeshRendering::new(
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

        let mut transform = Transform::default();
        transform.rotate(
            f32::to_radians(-90.0),
            morrigu::components::transform::Axis::X,
        );

        context.ecs_manager.world.insert_resource(camera);
        context
            .ecs_manager
            .world
            .spawn((transform, mesh_rendering_ref.clone()));

        Self {
            input_texture,
            output_texture,
            material_ref,
            mesh_rendering_ref,
        }
    }
}

impl ApplicationState for CSTState {
    fn on_attach(&mut self, context: &mut morrigu::application::StateContext) {
        let compute_shader = ComputeShader::builder()
            .build_from_spirv_u8(
                include_bytes!("shaders/gen/sharpen.comp"),
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
        let layout = self.output_texture.lock().image_ref.lock().layout;

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
                    image_memory_barriers: vec![vk::ImageMemoryBarrier::builder()
                        .old_layout(layout)
                        .new_layout(layout)
                        .image(self.output_texture.lock().image_ref.lock().handle)
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ)
                        .build()],
                },
                context.renderer,
            )
            .expect("Failed to run compute shader");
    }

    fn on_update(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::StateContext,
    ) {
    }

    fn on_update_egui(
        &mut self,
        _dt: std::time::Duration,
        _context: &mut morrigu::application::EguiUpdateContext,
    ) {
    }

    fn on_drop(&mut self, _context: &mut morrigu::application::StateContext) {}
}
