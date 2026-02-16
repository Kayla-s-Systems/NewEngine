#![forbid(unsafe_op_in_unsafe_fn)]

pub use newengine_math::types::{
    Mat2, Mat3, Mat3A, Mat4, Quat, Vec2, Vec2Swizzles, Vec3, Vec3A, Vec3Swizzles, Vec4, Vec4Swizzles,
};

/// Common scalar type for engine math (float).
pub type Scalar = f32;