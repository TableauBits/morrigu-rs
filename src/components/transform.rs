use std::ops::Mul;

use crate::{
    math_types::{Mat4, Quat, Vec3},
    utils::ThreadSafeRef,
};

#[derive(Debug, Clone, Copy)]
struct CacheData {
    pub is_outdated: bool,
    matrix: Mat4,
}

#[derive(Debug, Clone, bevy_ecs::component::Component)]
pub struct Transform {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,

    cache: ThreadSafeRef<CacheData>, // Necessary for interior mutability and MT
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::default(),
            rotation: Quat::default(),
            scale: Vec3::ONE,
            cache: ThreadSafeRef::new(CacheData {
                is_outdated: false,
                matrix: Mat4::IDENTITY,
            }),
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
            cache: ThreadSafeRef::new(CacheData {
                is_outdated: false,
                matrix: value,
            }),
        }
    }
}

impl From<Transform> for Mat4 {
    fn from(value: Transform) -> Self {
        value.cache.lock().matrix
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
        let mut cache_data = self.cache.lock();
        if cache_data.is_outdated {
            cache_data.matrix =
                Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
            cache_data.is_outdated = false;
        }

        cache_data.matrix
    }

    pub fn set_translation(&mut self, translation: &Vec3) {
        self.translation = *translation;
        self.cache.lock().is_outdated = true;
    }
    pub fn set_rotation(&mut self, rotation: &Quat) {
        self.rotation = *rotation;
        self.cache.lock().is_outdated = true;
    }
    pub fn set_scale(&mut self, scale: &Vec3) {
        self.scale = *scale;
        self.cache.lock().is_outdated = true;
    }

    pub fn translate(&mut self, translation: &Vec3) {
        self.translation += *translation;
        self.cache.lock().is_outdated = true;
    }
    pub fn rotate(&mut self, rotation: &Quat) {
        self.rotation *= *rotation;
        self.cache.lock().is_outdated = true;
    }
    pub fn rescale(&mut self, scale: &Vec3) {
        self.scale *= *scale;
        self.cache.lock().is_outdated = true;
    }
}

impl Mul<Transform> for Transform {
    type Output = Self;

    fn mul(self, rhs: Transform) -> Self::Output {
        let mat = self.matrix() * rhs.matrix();

        mat.into()
    }
}
