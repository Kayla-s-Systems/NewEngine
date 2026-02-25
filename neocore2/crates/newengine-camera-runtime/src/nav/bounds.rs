use newengine_math::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub struct BoundsSphere {
    pub center: Vec3,
    pub radius: f32,
}