#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::StableAbi;

#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MathDType {
    I32 = 1,
    I64 = 2,
    U32 = 3,
    U64 = 4,
    F32 = 5,
    F64 = 6,
    Bool = 7,
}

#[repr(u8)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MathTypeKind {
    Scalar = 1,
    Bytes = 2,
    Tensor = 3,
    Unit = 4,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MathType {
    pub kind: MathTypeKind,
    pub dtype: MathDType,
}

impl MathType {
    #[inline]
    pub const fn scalar(dtype: MathDType) -> Self {
        Self { kind: MathTypeKind::Scalar, dtype }
    }

    #[inline]
    pub const fn bytes() -> Self {
        Self { kind: MathTypeKind::Bytes, dtype: MathDType::U32 }
    }

    #[inline]
    pub const fn tensor(dtype: MathDType) -> Self {
        Self { kind: MathTypeKind::Tensor, dtype }
    }

    #[inline]
    pub const fn unit() -> Self {
        Self { kind: MathTypeKind::Unit, dtype: MathDType::U32 }
    }
}