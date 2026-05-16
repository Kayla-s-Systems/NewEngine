#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsMaterialDesc {
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
}

impl Default for PhysicsMaterialDesc {
    #[inline]
    fn default() -> Self {
        Self { friction: 0.75, restitution: 0.05, density: 1.0 }
    }
}

impl PhysicsMaterialDesc {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            friction: self.friction.clamp(0.0, 4.0),
            restitution: self.restitution.clamp(0.0, 1.0),
            density: self.density.clamp(0.0, 1_000_000.0),
        }
    }
}
