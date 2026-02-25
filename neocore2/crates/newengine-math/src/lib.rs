// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-wide math layer.
//!
//! # Goals
//! - Single dependency point for all math types and operations.
//! - Deterministic, auditable backend (custom scalar types).
//! - A dynamic registry for higher-level math routines (noise, intersections, special transforms),
//!   enabling plugins to extend/override functionality without leaking third-party math deps.

mod angle;
mod builtins;
pub mod collections;
mod euler;
mod ext;
mod gpu;
mod mat3;
mod mat4;
mod quat;
mod registry;
mod scalar;
mod value;
mod vec2;
mod vec3;
mod vec4;

#[macro_use]
mod macros;

pub use angle::wrap_pi;
pub use builtins::register_engine_builtins;
pub use euler::EulerRot;
pub use ext::{Vec2Ext, Vec3Ext};
pub use gpu::mat4_to_cols_bytes;
pub use mat3::Mat3;
pub use mat4::Mat4;
pub use quat::Quat;
pub use registry::{
    DynMathFn, MathFnId, MathRegistry, MathRegistryRef, ProviderId, RegisterMathFn,
};
pub use value::{MathError, MathResult, MathValue, MathValueType, Signature};
pub use vec2::Vec2;
pub use vec3::Vec3;
pub use vec4::Vec4;

/// Re-exported for use by `ne_math_fn!`.
pub use once_cell::sync::Lazy;

pub mod prelude {
    pub use super::wrap_pi;
    pub use super::{EulerRot, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
    pub use super::{Vec2Ext, Vec3Ext};

    #[cfg(feature = "collections")]
    pub use super::collections::prelude::*;
}

/// Prelude for engine-wide collection policies.
#[cfg(feature = "collections")]
pub mod collections_prelude {
    pub use crate::collections::prelude::*;
}
