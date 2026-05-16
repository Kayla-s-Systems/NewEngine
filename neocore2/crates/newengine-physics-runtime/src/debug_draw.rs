use newengine_math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsDebugLine {
    pub a: Vec3,
    pub b: Vec3,
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default)]
pub struct PhysicsDebugDrawFrame {
    pub lines: Vec<PhysicsDebugLine>,
}
