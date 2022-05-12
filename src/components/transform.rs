use nalgebra_glm as glm;

pub enum Axis {
    X,
    Y,
    Z,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bevy_ecs::component::Component)]
pub struct Transform {
    position: glm::Vec3,
    rotation: glm::Vec3,
    scale: glm::Vec3,

    cached_transform: glm::Mat4,
}

impl Transform {
    fn recompute_matrix(&mut self) {
        let translation_matrix = glm::translate(&glm::Mat4::identity(), &self.position);
        let rotation_matrix = glm::quat_to_mat4(&glm::quat(
            self.position.x,
            self.position.y,
            self.position.z,
            1.0,
        ));
        let scale_matrix = glm::scale(&glm::Mat4::identity(), &self.scale);
        self.cached_transform = translation_matrix * rotation_matrix * scale_matrix;
    }

    pub fn set_position(&mut self, position: &glm::Vec3) {
        self.position = *position;
        self.recompute_matrix();
    }

    pub fn translate(&mut self, translation: &glm::Vec3) {
        self.position += translation;
        self.recompute_matrix();
    }

    pub fn set_rotation(&mut self, rotation: &glm::Vec3) {
        self.rotation = *rotation;
        self.recompute_matrix();
    }

    // @TODO(Ithyx): Rework this to allow rotation on arbitrary axis
    pub fn rotate(&mut self, rotation: f32, axis: Axis) {
        let axis_rotation = match axis {
            Axis::X => &mut self.rotation.x,
            Axis::Y => &mut self.rotation.y,
            Axis::Z => &mut self.rotation.z,
        };

        *axis_rotation += rotation;
    }

    pub fn set_scale(&mut self, scale: &glm::Vec3) {
        self.scale = *scale;
        self.recompute_matrix();
    }

    pub fn scale(&mut self, scale: &glm::Vec3) {
        self.scale = self.scale.component_mul(scale);
        self.recompute_matrix();
    }

    pub fn matrix(&self) -> &glm::Mat4 {
        &self.cached_transform
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Default::default(),
            rotation: Default::default(),
            scale: Default::default(),
            cached_transform: glm::Mat4::identity(),
        }
    }
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
