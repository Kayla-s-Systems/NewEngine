#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_math::{ne_math_fn, MathError, MathRegistry, MathValue, MathValueType, Signature};

ne_math_fn!(
    Vec2DotA,
    "test.vec2.dot.v1",
    [Vec2, Vec2] => F32,
    |a: newengine_math::Vec2, b: newengine_math::Vec2| { a.dot(b) }
);

ne_math_fn!(
    Vec2DotB,
    "test.vec2.dot.v1",
    [Vec2, Vec2] => F32,
    |a: newengine_math::Vec2, b: newengine_math::Vec2| { a.dot(b) + 1.0 }
);

ne_math_fn!(
    Vec2DotWrongSig,
    "test.vec2.dot.v1",
    [Vec2] => F32,
    |a: newengine_math::Vec2| { a.length() }
);

#[test]
fn register_and_override_same_id_same_signature() {
    let reg = MathRegistry::default();

    reg.register(Arc::<str>::from("p1"), Arc::new(Vec2DotA))
        .unwrap();

    // Override by another provider becomes active.
    reg.register(Arc::<str>::from("p2"), Arc::new(Vec2DotB))
        .unwrap();

    let a = newengine_math::Vec2::new(1.0, 2.0);
    let b = newengine_math::Vec2::new(3.0, 4.0);
    let out = reg
        .call("test.vec2.dot.v1", &[MathValue::Vec2(a), MathValue::Vec2(b)])
        .unwrap();

    match out {
        MathValue::F32(v) => assert_eq!(v, a.dot(b) + 1.0),
        _ => panic!("unexpected return type"),
    }
}

#[test]
fn register_rejects_signature_conflict_for_same_id() {
    let reg = MathRegistry::default();

    reg.register(Arc::<str>::from("p1"), Arc::new(Vec2DotA))
        .unwrap();

    let err = reg
        .register(Arc::<str>::from("p1"), Arc::new(Vec2DotWrongSig))
        .unwrap_err();

    match err {
        MathError::SignatureConflict {
            id,
            expected,
            got,
            provider,
        } => {
            assert_eq!(id, "test.vec2.dot.v1");
            assert_eq!(provider, "p1");
            assert_eq!(
                expected,
                Signature {
                    inputs: vec![MathValueType::Vec2, MathValueType::Vec2],
                    output: MathValueType::F32,
                }
            );
            assert_eq!(
                got,
                Signature {
                    inputs: vec![MathValueType::Vec2],
                    output: MathValueType::F32,
                }
            );
        }
        _ => panic!("unexpected error: {err:?}"),
    }
}

#[test]
fn call_reports_mismatching_argument_index() {
    let reg = MathRegistry::default();
    reg.register(Arc::<str>::from("p1"), Arc::new(Vec2DotA))
        .unwrap();

    let err = reg
        .call(
            "test.vec2.dot.v1",
            &[MathValue::Vec2(newengine_math::Vec2::ZERO), MathValue::F32(1.0)],
        )
        .unwrap_err();

    match err {
        MathError::InvalidArgs { arg_index, .. } => assert_eq!(arg_index, Some(1)),
        _ => panic!("unexpected error: {err:?}"),
    }
}
