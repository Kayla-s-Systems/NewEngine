#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;

/// Immutable viewport descriptor.
///
/// Contains only logical information.
/// No GPU state here.
#[derive(Clone, Debug)]
pub struct ViewportDescriptor {
    /// Camera entity used to render this viewport.
    /// `None` means rendering must be skipped.
    pub camera: Option<EntityId>,
}

impl ViewportDescriptor {
    #[inline]
    pub fn new(camera: Option<EntityId>) -> Self {
        Self { camera }
    }
}
