#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use crate::{
    DynMathFn, MathRegistry, MathResult, MathValue, ProviderId,
};

/// Registers a minimal set of engine-provided math routines.
///
/// Call this once during engine startup.
pub fn register_engine_builtins(reg: &MathRegistry) -> MathResult<()> {
    let provider: ProviderId = Arc::<str>::from("newengine.core");

    reg.register_many(
        provider,
        [
            Arc::new(Vec3Dot) as Arc<dyn DynMathFn>,
            Arc::new(Vec3Cross) as Arc<dyn DynMathFn>,
        ],
    )?;

    Ok(())
}

crate::ne_math_fn!(Vec3Dot, "newengine.math.vec3.dot.v1", [Vec3, Vec3] -> F32, |args| {
    let (a, b) = match args {
        [MathValue::Vec3(a), MathValue::Vec3(b)] => (*a, *b),
        _ => unreachable!("signature validated by MathRegistry"),
    };
    Ok(MathValue::F32(a.dot(b)))
});

crate::ne_math_fn!(Vec3Cross, "newengine.math.vec3.cross.v1", [Vec3, Vec3] -> Vec3, |args| {
    let (a, b) = match args {
        [MathValue::Vec3(a), MathValue::Vec3(b)] => (*a, *b),
        _ => unreachable!("signature validated by MathRegistry"),
    };
    Ok(MathValue::Vec3(a.cross(b)))
});
