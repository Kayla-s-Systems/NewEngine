#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-wide math layer.
//!
//! # Goals
//! - Single dependency point for all math types and operations.
//! - Deterministic, auditable backend (today: `glam` re-export; later: custom).
//! - A dynamic registry for higher-level math routines (noise, intersections, special transforms),
//!   enabling plugins to extend/override functionality without leaking third-party math deps.

mod registry;
mod value;
mod macros;
mod builtins;

pub use builtins::register_engine_builtins;
pub use registry::{DynMathFn, MathFnId, MathRegistry, MathRegistryRef, ProviderId, RegisterMathFn};
pub use value::{MathError, MathResult, MathValue, MathValueType, Signature};

// -------------------------------------------------------------------------------------------------
// Temporary backend (glam).
// -------------------------------------------------------------------------------------------------

#[cfg(feature = "backend-glam")]
pub use glam::{EulerRot, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

#[cfg(feature = "backend-glam")]
pub mod prelude {
    pub use super::{EulerRot, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
}
