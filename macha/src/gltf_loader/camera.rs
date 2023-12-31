use std::time::Duration;

use morrigu::{
    components::camera::Camera,
    math_types::{Vec2, Vec3},
};
use winit::keyboard::KeyCode;
use winit_input_helper::WinitInputHelper;

pub struct ViewerCamera {
    pub mrg_camera: Camera,
    pub move_speed: f32,
    pub mouse_input_factor: f32,

    distance: f32,
    focal_point: Vec3,
}

#[profiling::all_functions]
impl ViewerCamera {
    pub fn new(mrg_camera: Camera) -> Self {
        let focal_point = Default::default();

        let mut new_camera = Self {
            mrg_camera,
            move_speed: 1.0,
            distance: 1.0,
            mouse_input_factor: 0.003,
            focal_point,
        };

        new_camera.set_focal_point(&focal_point);

        new_camera
    }

    #[profiling::skip]
    pub fn focal_point(&self) -> &Vec3 {
        &self.focal_point
    }

    pub fn set_focal_point(&mut self, new_focal_point: &Vec3) {
        self.focal_point = *new_focal_point;
        let forward = self.mrg_camera.forward_vector();
        let new_position = self.focal_point - forward * self.distance;
        self.mrg_camera.set_position(&new_position);
    }

    #[allow(unused)]
    #[profiling::skip]
    pub fn distance(&self) -> &f32 {
        &self.distance
    }

    pub fn set_distance(&mut self, new_distance: f32) {
        self.distance = new_distance.clamp(0.5, 100.0);
        let forward = self.mrg_camera.forward_vector();
        let new_position = self.focal_point - forward * self.distance;
        self.mrg_camera.set_position(&new_position);
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        self.mrg_camera.on_resize(width, height);
    }

    pub fn on_update(&mut self, dt: Duration, input: &WinitInputHelper) {
        let diff = input.mouse_diff();
        let mouse_delta = Vec2::new(diff.0, -diff.1) * self.mouse_input_factor;

        #[repr(usize)]
        enum MouseButton {
            Left = 0,
            Right = 1,
            Middle = 2,
        }
        if input.mouse_held(MouseButton::Left as usize) {
            self.mouse_rotate(&mouse_delta);
        }
        if input.mouse_held(MouseButton::Right as usize) {
            self.mouse_zoom(mouse_delta.y * 5.0);
        }
        if input.mouse_held(MouseButton::Middle as usize) {
            self.mouse_pan(&mouse_delta);
        }

        let scroll = input.scroll_diff().1;
        if scroll != 0.0 {
            self.mouse_zoom(scroll * 0.4);
        }

        if input.key_held(KeyCode::KeyW) {
            let forward = self.mrg_camera.forward_vector();
            let new_focal_point =
                *self.focal_point() + forward * dt.as_secs_f32() * self.move_speed;
            self.set_focal_point(&new_focal_point);
        }

        if input.key_held(KeyCode::KeyS) {
            let forward = self.mrg_camera.forward_vector();
            let new_focal_point =
                *self.focal_point() - forward * dt.as_secs_f32() * self.move_speed;
            self.set_focal_point(&new_focal_point);
        }

        if input.key_held(KeyCode::KeyA) {
            let right = self.mrg_camera.right_vector();
            let new_focal_point = *self.focal_point() + right * dt.as_secs_f32() * self.move_speed;
            self.set_focal_point(&new_focal_point);
        }

        if input.key_held(KeyCode::KeyD) {
            let right = self.mrg_camera.right_vector();
            let new_focal_point = *self.focal_point() - right * dt.as_secs_f32() * self.move_speed;
            self.set_focal_point(&new_focal_point);
        }

        if input.key_held(KeyCode::KeyQ) {
            let up = self.mrg_camera.up_vector();
            let new_focal_point = *self.focal_point() + up * dt.as_secs_f32() * self.move_speed;
            self.set_focal_point(&new_focal_point);
        }

        if input.key_held(KeyCode::KeyE) {
            let up = self.mrg_camera.up_vector();
            let new_focal_point = *self.focal_point() - up * dt.as_secs_f32() * self.move_speed;
            self.set_focal_point(&new_focal_point);
        }
    }

    fn mouse_rotate(&mut self, delta: &Vec2) {
        let new_pitch = self.mrg_camera.pitch() + -delta.x * 0.8;
        self.mrg_camera.set_pitch(new_pitch);

        let new_roll = self.mrg_camera.roll() + delta.y * 0.8;
        self.mrg_camera.set_roll(new_roll);

        let new_position = *self.focal_point() - self.mrg_camera.forward_vector() * self.distance;
        self.mrg_camera.set_position(&new_position);
    }

    fn mouse_zoom(&mut self, delta: f32) {
        let capped_distance_unit = f32::max(self.distance * 0.2, 0.0);
        let capped_speed = f32::min(capped_distance_unit * capped_distance_unit, 100.0);

        let clamped_distance = (self.distance - delta * capped_speed).clamp(0.1, 100.0);
        self.distance = clamped_distance;

        let new_position = *self.focal_point() - self.mrg_camera.forward_vector() * self.distance;
        self.mrg_camera.set_position(&new_position);
    }

    fn mouse_pan(&mut self, delta: &Vec2) {
        let x_pan_unit = f32::min(self.mrg_camera.size().x / 1000.0, 2.4);
        let x_pan_speed = 0.0366 * (x_pan_unit * x_pan_unit) - 0.1778 * x_pan_unit + 0.3021;
        let y_pan_unit = f32::min(self.mrg_camera.size().y / 1000.0, 2.4);
        let y_pan_speed = 0.0366 * (y_pan_unit * y_pan_unit) - 0.1778 * y_pan_unit + 0.3021;

        let mut new_focal_point = *self.focal_point();
        new_focal_point += self.mrg_camera.right_vector() * delta.x * x_pan_speed * self.distance;
        new_focal_point += self.mrg_camera.up_vector() * delta.y * y_pan_speed * self.distance;
        self.set_focal_point(&new_focal_point);
    }
}
