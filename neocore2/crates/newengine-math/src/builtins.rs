#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use crate::{
    DynMathFn, MathRegistry, MathResult, MathValue, MathValueType, ProviderId, Signature,
};

/// Registers a minimal set of engine-provided math routines.
///
/// Call this once during engine startup.
pub fn register_engine_builtins(reg: &MathRegistry) -> MathResult<()> {
    let provider: ProviderId = Arc::<str>::from("newengine.core");

    reg.register(provider.clone(), Arc::new(Vec3Dot))?;
    reg.register(provider.clone(), Arc::new(Vec3Cross))?;

    Ok(())
}

struct Vec3Dot;

impl DynMathFn for Vec3Dot {
    fn id(&self) -> &str {
        "newengine.math.vec3.dot.v1"
    }

    fn signature(&self) -> &Signature {
        static SIG: once_cell::sync::Lazy<Signature> = once_cell::sync::Lazy::new(|| Signature {
            inputs: vec![MathValueType::Vec3, MathValueType::Vec3],
            output: MathValueType::F32,
        });
        &SIG
    }

    fn invoke(&self, args: &[MathValue]) -> MathResult<MathValue> {
        let (a, b) = match args {
            [MathValue::Vec3(a), MathValue::Vec3(b)] => (*a, *b),
            _ => unreachable!("signature validated by MathRegistry"),
        };
        Ok(MathValue::F32(a.dot(b)))
    }
}

struct Vec3Cross;

impl DynMathFn for Vec3Cross {
    fn id(&self) -> &str {
        "newengine.math.vec3.cross.v1"
    }

    fn signature(&self) -> &Signature {
        static SIG: once_cell::sync::Lazy<Signature> = once_cell::sync::Lazy::new(|| Signature {
            inputs: vec![MathValueType::Vec3, MathValueType::Vec3],
            output: MathValueType::Vec3,
        });
        &SIG
    }

    fn invoke(&self, args: &[MathValue]) -> MathResult<MathValue> {
        let (a, b) = match args {
            [MathValue::Vec3(a), MathValue::Vec3(b)] => (*a, *b),
            _ => unreachable!("signature validated by MathRegistry"),
        };
        Ok(MathValue::Vec3(a.cross(b)))
    }
}
