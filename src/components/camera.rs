use bevy_ecs::system::Resource;

use std::default::Default;

use crate::{
    math_types::Quat,
    math_types::{Mat4, Vec2, Vec3},
};

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
    pub position: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

impl CameraBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    #[profiling::function]
    pub fn build(self, projection_type: Projection, size: &Vec2) -> Camera {
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

#[derive(Debug, Clone, Copy, Resource)]
pub struct Camera {
    projection_type: Projection,
    aspect_ratio: f32,
    position: Vec3,

    pitch: f32,
    yaw: f32,
    roll: f32,
    orientation: Quat,

    projection: Mat4,
    view: Mat4,
    view_projection: Mat4,

    size: Vec2,
}

#[profiling::all_functions]
impl Camera {
    #[profiling::skip]
    pub fn builder() -> CameraBuilder {
        CameraBuilder::new()
    }

    fn compute_orientation(pitch: f32, yaw: f32, roll: f32) -> Quat {
        Quat::from_euler(glam::EulerRot::YZX, pitch, yaw, roll)
    }

    fn compute_projection(projection_type: &Projection, aspect_ratio: f32) -> Mat4 {
        match projection_type {
            Projection::Perspective(data) => Mat4::perspective_rh(
                data.horizontal_fov,
                aspect_ratio,
                data.near_plane,
                data.far_plane,
            ),
            Projection::Orthographic(data) => {
                let right = data.scale * aspect_ratio * 0.5;
                let left = -right;

                let top = data.scale * 0.5;
                let bottom = -top;

                Mat4::orthographic_rh(left, right, bottom, top, data.near_plane, data.far_plane)
            }
        }
    }

    fn compute_view(position: &Vec3, orientation: &Quat) -> Mat4 {
        let view_inverse = Mat4::from_rotation_translation(*orientation, *position);
        view_inverse.inverse()
    }

    pub fn compute_view_projection(view: &Mat4, projection: &Mat4) -> Mat4 {
        (*projection) * (*view)
    }

    #[profiling::skip]
    pub fn view(&self) -> &Mat4 {
        &self.view
    }

    #[profiling::skip]
    pub fn projection(&self) -> &Mat4 {
        &self.projection
    }

    #[profiling::skip]
    pub fn view_projection(&self) -> &Mat4 {
        &self.view_projection
    }

    #[profiling::skip]
    pub fn position(&self) -> &Vec3 {
        &self.position
    }

    #[profiling::skip]
    pub fn pitch(&self) -> &f32 {
        &self.pitch
    }

    #[profiling::skip]
    pub fn yaw(&self) -> &f32 {
        &self.yaw
    }

    #[profiling::skip]
    pub fn roll(&self) -> &f32 {
        &self.roll
    }

    #[profiling::skip]
    pub fn aspect_ratio(&self) -> &f32 {
        &self.aspect_ratio
    }

    #[profiling::skip]
    pub fn size(&self) -> &Vec2 {
        &self.size
    }

    pub fn set_projection_type(&mut self, projection_type: Projection) {
        self.projection_type = projection_type;
        self.projection = Self::compute_projection(&self.projection_type, self.aspect_ratio);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection);
    }

    pub fn set_size(&mut self, size: &Vec2) {
        self.size = *size;

        let aspect_ratio = size.x / size.y;
        self.aspect_ratio = aspect_ratio;
        self.projection = Self::compute_projection(&self.projection_type, self.aspect_ratio);
        self.view_projection = Self::compute_view_projection(&self.view, &self.projection);
    }

    pub fn set_position(&mut self, position: &Vec3) {
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

    pub fn forward_vector(&self) -> Vec3 {
        Self::compute_orientation(self.pitch, self.yaw, self.roll).mul_vec3(Vec3::NEG_Z)
    }

    pub fn right_vector(&self) -> Vec3 {
        Self::compute_orientation(self.pitch, self.yaw, self.roll).mul_vec3(Vec3::NEG_X)
    }

    pub fn up_vector(&self) -> Vec3 {
        Self::compute_orientation(self.pitch, self.yaw, self.roll).mul_vec3(Vec3::NEG_Y)
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        self.set_size(&Vec2::new(width as f32, height as f32));
    }
}
