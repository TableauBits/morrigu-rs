use nalgebra::{Scale3, Transform3, Translation3, UnitQuaternion};

use crate::vector_type::Mat4;

#[derive(bevy_ecs::component::Component)]
pub struct Transform {
    translation: Translation3<f32>,
    rotation: UnitQuaternion<f32>,
    scale: Scale3<f32>,

    transform: nalgebra::Transform3<f32>,
}

impl From<nalgebra::Transform3<f32>> for Transform {
    fn from(value: nalgebra::Transform3<f32>) -> Self {
        let intermediate: nalgebra::Affine3<f32> =
            nalgebra::try_convert(value).expect("Invalid transform in gltf!");

        alga::linear::AffineTransformation::decompose(&intermediate);
        Self {
            translation: nalgebra::try_convert(value).unwrap(),
            rotation: nalgebra::try_convert(value).unwrap(),
            scale: nalgebra::try_convert(value).unwrap(),
            transform: value,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Translation3::default(),
            rotation: UnitQuaternion::default(),
            scale: Scale3::new(1.0, 1.0, 1.0),
            transform: nalgebra::Transform::default(),
        }
    }
}

impl From<Transform> for nalgebra::Transform3<f32> {
    fn from(val: Transform) -> Self {
        val.transform
    }
}

impl Transform {
    pub fn from_matrix(matrix: Mat4) -> Self {
        nalgebra::Transform3::<f32>::from_matrix_unchecked(matrix).into()
    }

    pub fn matrix(&self) -> &Mat4 {
        self.transform.matrix()
    }
    pub fn update_matrix(&mut self) {
        let intermediate: Transform3<f32> = nalgebra::convert(self.translation * self.rotation);
        self.transform = intermediate * nalgebra::convert::<_, Transform3<f32>>(self.scale);
    }

    pub fn translation(&self) -> &Translation3<f32> {
        &self.translation
    }
    pub fn set_translation(&mut self, translation: &Translation3<f32>) {
        self.translation = *translation;
    }
    pub fn set_translation_and_update(&mut self, translation: &Translation3<f32>) {
        self.set_translation(translation);
        self.update_matrix();
    }
    pub fn re_translate(&mut self, translation: &Translation3<f32>) {}

    pub fn rotation(&self) -> &UnitQuaternion<f32> {
        &self.rotation
    }
    pub fn set_rotation(&mut self, rotation: &UnitQuaternion<f32>) {
        self.rotation = *rotation;
    }
    pub fn set_rotation_and_update(&mut self, rotation: &UnitQuaternion<f32>) {
        self.set_rotation(rotation);
        self.update_matrix();
    }

    pub fn scale(&self) -> &Scale3<f32> {
        &self.scale
    }
    pub fn set_scale(&mut self, scale: &Scale3<f32>) {
        self.scale = *scale;
    }
    pub fn set_scalei_and_update(&mut self, scale: &Scale3<f32>) {
        self.set_scale(scale);
        self.update_matrix();
    }
}
