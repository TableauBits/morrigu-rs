use nalgebra_glm as glm;
use nalgebra as na;

use std::default::Default;

#[derive(Debug, Clone, Copy)]
pub struct PerspectiveData {
    pub horizontal_fov: f32,
    pub near_plane: f32,
    pub far_plane: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct OrthographicData {
    pub scale: f32,
    pub near_plane: f32,
    pub far_plane: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum Projection {
    Perspective(PerspectiveData),
    Orthographic(OrthographicData),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CameraBuilder {
    pub position: glm::Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

impl CameraBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn build(self, projection_type: Projection, size: &glm::Vec2) -> Camera {
        let orientation = Camera::compute_orientation(self.pitch, self.yaw, self.roll);

        let aspect_ratio = size.x / size.y;
        let projection = Camera::compute_projection(&projection_type, aspect_ratio);
        let view = Camera::compute_view(&self.position, &orientation);
        let view_projection = Camera::compute_view_projection(&view, &projection);

        Camera {
            projection_type,
            aspect_ratio,
            position: self.position,

            pitch: self.pitch,
            yaw: self.yaw,
            roll: self.roll,
            orientation,

            projection,
            view,
            view_projection,

            size: *size,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    projection_type: Projection,
    aspect_ratio: f32,
    position: glm::Vec3,

    pitch: f32,
    yaw: f32,
    roll: f32,
    orientation: na::UnitQuaternion<f32>,

    projection: glm::Mat4,
    view: glm::Mat4,
    view_projection: glm::Mat4,

    size: glm::Vec2,
}

impl Camera {
    pub fn builder() -> CameraBuilder {
        CameraBuilder::new()
    }

    fn compute_orientation(pitch: f32, yaw: f32, roll: f32) -> na::UnitQuaternion<f32> {
        na::UnitQuaternion::from_euler_angles(roll, pitch, yaw)
    }

    fn compute_projection(projection_type: &Projection, aspect_ratio: f32) -> glm::Mat4 {
        match projection_type {
            Projection::Perspective(data) => glm::perspective(
                aspect_ratio,
                data.horizontal_fov,
                data.near_plane,
                data.far_plane,
            ),
            Projection::Orthographic(data) => {
                let right = data.scale * aspect_ratio * 0.5;
                let left = -right;

                let top = data.scale * 0.5;
                let bottom = -top;

                glm::ortho(left, right, bottom, top, data.near_plane, data.far_plane)
            }
        }
    }

    fn compute_view(position: &glm::Vec3, orientation: &na::UnitQuaternion<f32>) -> glm::Mat4 {
        let view_inverse =
            glm::translate(&glm::Mat4::identity(), position) * glm::quat_to_mat4(orientation);
        glm::inverse(&view_inverse)
    }

    pub fn compute_view_projection(view: &glm::Mat4, projection: &glm::Mat4) -> glm::Mat4 {
        projection * view
    }

    pub fn view(&self) -> &glm::Mat4 {
        &self.view
    }

    pub fn projection(&self) -> &glm::Mat4 {
        &self.projection
    }

    pub fn view_projection(&self) -> &glm::Mat4 {
        &self.view_projection
    }

    pub fn position(&self) -> &glm::Vec3 {
        &self.position
    }

    pub fn pitch(&self) -> &f32 {
        &self.pitch
    }

    pub fn yaw(&self) -> &f32 {
        &self.yaw
    }

    pub fn roll(&self) -> &f32 {
        &self.roll
    }

    pub fn size(&self) -> &glm::Vec2 {
        &self.size
    }

    pub fn set_projection_type(&mut self, projection_type: Projection) {
        self.projection_type = projection_type;
        self.projection = Self::compute_projection(&self.projection_type, self.aspect_ratio);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection);
    }

    pub fn set_size(&mut self, size: &glm::Vec2) {
        self.size = *size;

        let aspect_ratio = size.x / size.y;
        self.aspect_ratio = aspect_ratio;
        self.projection = Self::compute_projection(&self.projection_type, self.aspect_ratio);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection);
    }

    pub fn set_position(&mut self, position: &glm::Vec3) {
        self.position = *position;
        self.view = Self::compute_view(&self.position, &self.orientation);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection)
    }

    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch;
        self.orientation = Self::compute_orientation(self.pitch, self.yaw, self.roll);
        self.view = Self::compute_view(&self.position, &self.orientation);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection)
    }

    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = yaw;
        self.orientation = Self::compute_orientation(self.pitch, self.yaw, self.roll);
        self.view = Self::compute_view(&self.position, &self.orientation);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection)
    }

    pub fn set_roll(&mut self, roll: f32) {
        self.roll = roll;
        self.orientation = Self::compute_orientation(self.pitch, self.yaw, self.roll);
        self.view = Self::compute_view(&self.position, &self.orientation);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection)
    }

    pub fn forward_vector(&self) -> glm::Vec3 {
        glm::quat_rotate_vec3(
            &Self::compute_orientation(self.pitch, self.yaw, self.roll),
            &glm::vec3(0.0, 0.0, -1.0),
        )
    }

    pub fn right_vector(&self) -> glm::Vec3 {
        glm::quat_rotate_vec3(
            &Self::compute_orientation(self.pitch, self.yaw, self.roll),
            &glm::vec3(-1.0, 0.0, 0.0),
        )
    }

    pub fn up_vector(&self) -> glm::Vec3 {
        glm::quat_rotate_vec3(
            &Self::compute_orientation(self.pitch, self.yaw, self.roll),
            &glm::vec3(0.0, -1.0, 0.0),
        )
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        self.set_size(&glm::vec2(width as f32, height as f32));
    }
}
