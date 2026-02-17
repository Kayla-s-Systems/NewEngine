#![forbid(unsafe_op_in_unsafe_fn)]

/// Macro for defining a `DynMathFn` with minimal boilerplate.
///
/// # Example
/// ```
/// use newengine_math::{MathRegistry, ProviderId, MathValue, ne_math_fn};
///
/// ne_math_fn!(Vec3Dot, "k-sys.math.vec3.dot.v1", [Vec3, Vec3] -> F32, |args| {
///     let (a, b) = match args {
///         [MathValue::Vec3(a), MathValue::Vec3(b)] => (*a, *b),
///         _ => unreachable!("signature validated by MathRegistry"),
///     };
///     Ok(MathValue::F32(a.dot(b)))
/// });
/// ```
#[macro_export]
macro_rules! ne_math_fn {
    (
        $name:ident,
        $id:literal,
        [ $( $in_ty:ident ),* $(,)? ] -> $out_ty:ident,
        |$args:ident| $body:block
    ) => {
        struct $name;

        impl $crate::DynMathFn for $name {
            #[inline]
            fn id(&self) -> &str {
                $id
            }

            #[inline]
            fn signature(&self) -> &$crate::Signature {
                static SIG: once_cell::sync::Lazy<$crate::Signature> = once_cell::sync::Lazy::new(|| $crate::Signature {
                    inputs: vec![ $( $crate::MathValueType::$in_ty ),* ],
                    output: $crate::MathValueType::$out_ty,
                });
                &SIG
            }

            #[inline]
            fn invoke(&self, $args: &[$crate::MathValue]) -> $crate::MathResult<$crate::MathValue> {
                $body
            }
        }
    };
}
