use std::cell::RefCell;

use crate::{
    math_types::{Mat4, Quat, Vec3},
    utils::ThreadSafeRef,
};

#[derive(Debug, Clone, bevy_ecs::component::Component)]
pub struct Transform {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,

    is_cache_outdated: bool,
    cached_matrix: ThreadSafeRef<Mat4>, // Necessary for interior mutability and MT
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::default(),
            rotation: Quat::default(),
            scale: Vec3::ONE,
            is_cache_outdated: false,
            cached_matrix: ThreadSafeRef::new(Mat4::IDENTITY),
        }
    }
}

impl From<Mat4> for Transform {
    fn from(value: Mat4) -> Self {
        let (scale, rotation, translation) = value.to_scale_rotation_translation();
        Self {
            translation,
            rotation,
            scale,
            is_cache_outdated: false,
            cached_matrix: ThreadSafeRef::new(value),
        }
    }
}

impl From<Transform> for Mat4 {
    fn from(value: Transform) -> Self {
        *value.cached_matrix.lock()
    }
}

impl Transform {
    pub fn translation(&self) -> &Vec3 {
        &self.translation
    }
    pub fn rotation(&self) -> &Quat {
        &self.rotation
    }
    pub fn scale(&self) -> &Vec3 {
        &self.scale
    }
    pub fn matrix(&self) -> Mat4 {
        if self.is_cache_outdated {
            *self.cached_matrix.lock() =
                Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        }

        *self.cached_matrix.lock()
    }

    pub fn set_translation(&mut self, translation: &Vec3) {
        self.translation = *translation;
        self.is_cache_outdated = true;
    }
    pub fn set_rotation(&mut self, rotation: &Quat) {
        self.rotation = *rotation;
        self.is_cache_outdated = true;
    }
    pub fn set_scale(&mut self, scale: &Vec3) {
        self.scale = *scale;
        self.is_cache_outdated = true;
    }

    pub fn translate(&mut self, translation: &Vec3) {
        self.translation += *translation;
        self.is_cache_outdated = true;
    }
    pub fn rotate(&mut self, rotation: &Quat) {
        self.rotation *= *rotation;
        self.is_cache_outdated = true;
    }
    pub fn rescale(&mut self, scale: &Vec3) {
        self.scale *= *scale;
        self.is_cache_outdated = true;
    }
}
