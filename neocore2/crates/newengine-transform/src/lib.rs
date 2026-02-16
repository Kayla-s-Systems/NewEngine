#![forbid(unsafe_op_in_unsafe_fn)]

mod components;
mod hierarchy;
mod propagate;

pub use components::{
    Children, GlobalTransform, Parent, Transform, TransformDirty, WorldPose,
};

pub use hierarchy::set_parent;

pub use propagate::{
    ensure_transform_outputs,
    propagate_transforms,
    TransformPropagationScratch,
};
