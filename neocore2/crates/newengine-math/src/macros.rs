// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

/// Defines a zero-sized dyn math function wrapper with argument checking.
///
/// Usage:
/// `ne_math_fn!(Name, "id", [Vec3, Vec3] => F32, |a: Vec3, b: Vec3| { a.dot(b) });`
#[macro_export]
macro_rules! ne_math_fn {
    (
        $name:ident,
        $id:expr,
        [$($in_ty:ident),* $(,)?] => $out_ty:ident,
        |$($arg:ident : $arg_ty:ty),* $(,)?| $body:block
    ) => {
        #[allow(non_camel_case_types)]
        pub struct $name;

        impl $name {
            #[inline]
            fn sig() -> &'static $crate::Signature {
                static SIG: $crate::Lazy<$crate::Signature> = $crate::Lazy::new(|| $crate::Signature {
                    inputs: vec![$($crate::MathValueType::$in_ty),*],
                    output: $crate::MathValueType::$out_ty,
                });
                &SIG
            }

            #[inline]
            fn invalid_args(args: &[$crate::MathValue]) -> $crate::MathError {
                $crate::MathError::InvalidArgs {
                    expected: Self::sig().clone(),
                    got: args.iter().map(|v| v.ty()).collect(),
                    arg_index: None,
                }
            }

            #[inline]
            fn to_value(v: $crate::ne_math_fn!(@out_rust_ty $out_ty, v)) -> $crate::MathValue {
                $crate::ne_math_fn!(@wrap_out $out_ty, v)
            }
        }

        impl $crate::DynMathFn for $name {
            #[inline]
            fn id(&self) -> &str {
                $id
            }

            #[inline]
            fn signature(&self) -> &$crate::Signature {
                Self::sig()
            }

            fn invoke(&self, args: &[$crate::MathValue]) -> $crate::MathResult<$crate::MathValue> {
                // Fast path: length check.
                const N: usize = $crate::ne_math_fn!(@count $($in_ty),*);
                if args.len() != N {
                    return Err(Self::invalid_args(args));
                }

                // Typed decode.
                let mut it = args.iter();
                $(
                    let $arg: $arg_ty = match it.next().expect("checked length") {
                        $crate::ne_math_fn!(@match_pat $in_ty, v) => $crate::ne_math_fn!(@match_get $in_ty, v),
                        _ => return Err(Self::invalid_args(args)),
                    };
                )*

                // Execute.
                let out = (|| $body)();
                Ok(Self::to_value(out))
            }
        }
    };

    // --- helpers ---

    (@count) => {0usize};
    (@count $head:ident $(, $tail:ident)*) => {1usize + $crate::ne_math_fn!(@count $($tail),*)};

    (@match_pat Unit, $v:ident) => { $crate::MathValue::Unit };
    (@match_pat Bool, $v:ident) => { $crate::MathValue::Bool($v) };
    (@match_pat I32,  $v:ident) => { $crate::MathValue::I32($v) };
    (@match_pat U32,  $v:ident) => { $crate::MathValue::U32($v) };
    (@match_pat F32,  $v:ident) => { $crate::MathValue::F32($v) };
    (@match_pat F64,  $v:ident) => { $crate::MathValue::F64($v) };
    (@match_pat Vec2, $v:ident) => { $crate::MathValue::Vec2($v) };
    (@match_pat Vec3, $v:ident) => { $crate::MathValue::Vec3($v) };
    (@match_pat Vec4, $v:ident) => { $crate::MathValue::Vec4($v) };
    (@match_pat Quat, $v:ident) => { $crate::MathValue::Quat($v) };
    (@match_pat Mat4, $v:ident) => { $crate::MathValue::Mat4($v) };
    (@match_pat Bytes, $v:ident) => { $crate::MathValue::Bytes($v) };
    (@match_pat String, $v:ident) => { $crate::MathValue::String($v) };

    (@match_get Unit, $v:ident) => { () };
    (@match_get Bool, $v:ident) => { *$v };
    (@match_get I32,  $v:ident) => { *$v };
    (@match_get U32,  $v:ident) => { *$v };
    (@match_get F32,  $v:ident) => { *$v };
    (@match_get F64,  $v:ident) => { *$v };
    (@match_get Vec2, $v:ident) => { *$v };
    (@match_get Vec3, $v:ident) => { *$v };
    (@match_get Vec4, $v:ident) => { *$v };
    (@match_get Quat, $v:ident) => { *$v };
    (@match_get Mat4, $v:ident) => { *$v };
    (@match_get Bytes, $v:ident) => { $v.clone() };
    (@match_get String, $v:ident) => { $v.clone() };

    (@wrap_out Unit, $v:expr) => { $crate::MathValue::Unit };
    (@wrap_out Bool, $v:expr) => { $crate::MathValue::Bool($v) };
    (@wrap_out I32,  $v:expr) => { $crate::MathValue::I32($v) };
    (@wrap_out U32,  $v:expr) => { $crate::MathValue::U32($v) };
    (@wrap_out F32,  $v:expr) => { $crate::MathValue::F32($v) };
    (@wrap_out F64,  $v:expr) => { $crate::MathValue::F64($v) };
    (@wrap_out Vec2, $v:expr) => { $crate::MathValue::Vec2($v) };
    (@wrap_out Vec3, $v:expr) => { $crate::MathValue::Vec3($v) };
    (@wrap_out Vec4, $v:expr) => { $crate::MathValue::Vec4($v) };
    (@wrap_out Quat, $v:expr) => { $crate::MathValue::Quat($v) };
    (@wrap_out Mat4, $v:expr) => { $crate::MathValue::Mat4($v) };
    (@wrap_out Bytes, $v:expr) => { $crate::MathValue::Bytes($v) };
    (@wrap_out String, $v:expr) => { $crate::MathValue::String($v) };

    // Rust output type mapping for the helper `to_value`.
    (@out_rust_ty Unit, $v:ident) => { () };
    (@out_rust_ty Bool, $v:ident) => { bool };
    (@out_rust_ty I32,  $v:ident) => { i32 };
    (@out_rust_ty U32,  $v:ident) => { u32 };
    (@out_rust_ty F32,  $v:ident) => { f32 };
    (@out_rust_ty F64,  $v:ident) => { f64 };
    (@out_rust_ty Vec2, $v:ident) => { $crate::Vec2 };
    (@out_rust_ty Vec3, $v:ident) => { $crate::Vec3 };
    (@out_rust_ty Vec4, $v:ident) => { $crate::Vec4 };
    (@out_rust_ty Quat, $v:ident) => { $crate::Quat };
    (@out_rust_ty Mat4, $v:ident) => { $crate::Mat4 };
    (@out_rust_ty Bytes, $v:ident) => { Vec<u8> };
    (@out_rust_ty String, $v:ident) => { String };
}
