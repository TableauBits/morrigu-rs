use nalgebra_glm as glm;

#[repr(C)]
#[derive(Debug, Clone, Copy, bevy_ecs::component::Component)]
pub struct Transform {
    pub position: glm::Vec3,
    pub rotation: glm::Vec3,
    pub scale: glm::Vec3,

    cached_transform: glm::Mat4,
}

impl std::fmt::Display for Transform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_fmt(format_args!(
            "{{ position: {}, rotation: {}, scale: {} }}",
            self.position, self.rotation, self.scale
        ))
    }
}

impl Transform {}
