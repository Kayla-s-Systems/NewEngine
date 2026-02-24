#![forbid(unsafe_op_in_unsafe_fn)]

mod components;
#[cfg(feature = "ecs")]
mod hierarchy;
#[cfg(feature = "ecs")]
mod propagate;

pub use components::{
    GlobalTransform, Transform, WorldPose,
};

#[cfg(feature = "ecs")]
pub use components::{Children, Parent, TransformDirty};

#[cfg(feature = "ecs")]
pub use hierarchy::set_parent;

#[cfg(feature = "ecs")]
pub use propagate::{
    ensure_transform_outputs, propagate_transforms, TransformPropagationScratch,
};
