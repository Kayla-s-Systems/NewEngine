// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use crate::{ne_math_fn, DynMathFn, Mat4, MathRegistry, MathResult, ProviderId, Quat, Vec2, Vec3};

/// Registers engine-provided math routines.
///
/// Call once during startup. Plugins may extend/override later.
pub fn register_engine_builtins(reg: &MathRegistry) -> MathResult<()> {
    let provider: ProviderId = Arc::<str>::from("newengine.core");

    reg.register_many(
        provider,
        [
            Arc::new(Vec2Dot) as Arc<dyn DynMathFn>,
            Arc::new(Vec2LenSq),
            Arc::new(Vec2Len),
            Arc::new(Vec2Normalize),
            Arc::new(Vec3Dot),
            Arc::new(Vec3Cross),
            Arc::new(Vec3LenSq),
            Arc::new(Vec3Len),
            Arc::new(Vec3Normalize),
            Arc::new(QuatMul),
            Arc::new(QuatRotateVec3),
            Arc::new(Mat4Mul),
            Arc::new(Mat4TransformPoint3),
            Arc::new(Mat4TransformVector3),
        ],
    )?;

    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Vec2
// -------------------------------------------------------------------------------------------------

ne_math_fn!(Vec2Dot, "newengine.math.vec2.dot.v1", [Vec2, Vec2] => F32, |a: Vec2, b: Vec2| {
    a.dot(b)
});

ne_math_fn!(Vec2LenSq, "newengine.math.vec2.length_sq.v1", [Vec2] => F32, |v: Vec2| {
    v.length_squared()
});

ne_math_fn!(Vec2Len, "newengine.math.vec2.length.v1", [Vec2] => F32, |v: Vec2| {
    v.length()
});

ne_math_fn!(Vec2Normalize, "newengine.math.vec2.normalize.v1", [Vec2] => Vec2, |v: Vec2| {
    let lsq = v.length_squared();
    if lsq <= 1e-20 {
        Vec2::ZERO
    } else {
        v * lsq.sqrt().recip()
    }
});

// -------------------------------------------------------------------------------------------------
// Vec3
// -------------------------------------------------------------------------------------------------

ne_math_fn!(Vec3Dot, "newengine.math.vec3.dot.v1", [Vec3, Vec3] => F32, |a: Vec3, b: Vec3| {
    a.dot(b)
});

ne_math_fn!(Vec3Cross, "newengine.math.vec3.cross.v1", [Vec3, Vec3] => Vec3, |a: Vec3, b: Vec3| {
    a.cross(b)
});

ne_math_fn!(Vec3LenSq, "newengine.math.vec3.length_sq.v1", [Vec3] => F32, |v: Vec3| {
    v.length_squared()
});

ne_math_fn!(Vec3Len, "newengine.math.vec3.length.v1", [Vec3] => F32, |v: Vec3| {
    v.length()
});

ne_math_fn!(Vec3Normalize, "newengine.math.vec3.normalize.v1", [Vec3] => Vec3, |v: Vec3| {
    let lsq = v.length_squared();
    if lsq <= 1e-20 {
        Vec3::ZERO
    } else {
        v * lsq.sqrt().recip()
    }
});

// -------------------------------------------------------------------------------------------------
// Quat
// -------------------------------------------------------------------------------------------------

ne_math_fn!(QuatMul, "newengine.math.quat.mul.v1", [Quat, Quat] => Quat, |a: Quat, b: Quat| {
    a * b
});

ne_math_fn!(QuatRotateVec3, "newengine.math.quat.rotate_vec3.v1", [Quat, Vec3] => Vec3, |q: Quat, v: Vec3| {
    q * v
});

// -------------------------------------------------------------------------------------------------
// Mat4
// -------------------------------------------------------------------------------------------------

ne_math_fn!(Mat4Mul, "newengine.math.mat4.mul.v1", [Mat4, Mat4] => Mat4, |a: Mat4, b: Mat4| {
    a * b
});

ne_math_fn!(Mat4TransformPoint3, "newengine.math.mat4.transform_point3.v1", [Mat4, Vec3] => Vec3, |m: Mat4, p: Vec3| {
    // Treat input as point (w=1).
    let v = m * crate::Vec4::new(p.x, p.y, p.z, 1.0);
    let inv_w = v.w.abs().max(1e-20).recip();
    Vec3::new(v.x * inv_w, v.y * inv_w, v.z * inv_w)
});

ne_math_fn!(Mat4TransformVector3, "newengine.math.mat4.transform_vec3.v1", [Mat4, Vec3] => Vec3, |m: Mat4, v: Vec3| {
    // Treat input as direction (w=0).
    let r = m * crate::Vec4::new(v.x, v.y, v.z, 0.0);
    Vec3::new(r.x, r.y, r.z)
});
