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
        let rotation_matrix = {
            let rot_x = glm::rotation(self.rotation.x, &glm::vec3(1.0, 0.0, 0.0));
            let rot_y = glm::rotation(self.rotation.y, &glm::vec3(0.0, 1.0, 0.0));
            let rot_z = glm::rotation(self.rotation.z, &glm::vec3(0.0, 0.0, 1.0));

            rot_x * rot_y * rot_z
        };
        let scale_matrix = glm::scale(&glm::Mat4::identity(), &self.scale);
        self.cached_transform = translation_matrix * rotation_matrix * scale_matrix;
    }

    pub fn set_position(&mut self, position: &glm::Vec3) -> &mut Self {
        self.position = *position;
        self.recompute_matrix();

        self
    }

    pub fn translate(&mut self, translation: &glm::Vec3) -> &mut Self {
        self.position += translation;
        self.recompute_matrix();

        self
    }

    pub fn set_rotation(&mut self, rotation: &glm::Vec3) -> &mut Self {
        self.rotation = *rotation;
        self.recompute_matrix();

        self
    }

    // @TODO(Ithyx): Rework this to allow rotation on arbitrary axis
    // Updating the transform is easy enough.
    // However, my brain is veri smol, so not sure how to update individual values
    pub fn rotate(&mut self, rotation: f32, axis: Axis) -> &mut Self {
        let axis_rotation = match axis {
            Axis::X => &mut self.rotation.x,
            Axis::Y => &mut self.rotation.y,
            Axis::Z => &mut self.rotation.z,
        };

        *axis_rotation += rotation;
        self.recompute_matrix();

        self
    }

    pub fn set_scale(&mut self, scale: &glm::Vec3) -> &mut Self {
        self.scale = *scale;
        self.recompute_matrix();

        self
    }

    pub fn scale(&mut self, scale: &glm::Vec3) -> &mut Self {
        self.scale = self.scale.component_mul(scale);
        self.recompute_matrix();

        self
    }

    pub fn set_matrix(&mut self, matrix: &glm::Mat4) -> &mut Self {
        self.cached_transform = *matrix;

        // @TODO(Ithyx)
        // Find a way to revert tranform matrix to it's original components
        // https://github.com/g-truc/glm/blob/master/glm/gtx/matrix_decompose.inl
        
        self
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
            scale: glm::vec3(1.0, 1.0, 1.0),
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
