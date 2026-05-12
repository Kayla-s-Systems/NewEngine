#![forbid(unsafe_op_in_unsafe_fn)]

mod resource;
mod types;

pub use resource::CameraManagerResource;
pub use types::{
    CameraDirectorKind, CameraDirectorRequest, CameraInputContext, CameraRuntimeMode,
    CameraRuntimeReport, CameraRuntimeWorldState, CameraTransitionPhase, CameraTransitionPlan,
    CameraTransitionState,
};
