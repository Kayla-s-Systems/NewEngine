#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RVec};
use abi_stable::StableAbi;

use crate::types::{MathDType, MathType, MathTypeKind};

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct TensorDesc {
    pub dtype: MathDType,
    pub shape: RVec<u32>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct TensorValue {
    pub desc: TensorDesc,
    pub data: RVec<u8>,
}

#[repr(u8)]
#[derive(StableAbi, Clone, Debug, PartialEq)]
pub enum MathValue {
    I32(i32),
    I64(i64),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Bytes(RVec<u8>),
    Tensor(TensorValue),
    Unit,
}

impl MathValue {
    #[inline]
    pub fn math_type(&self) -> MathType {
        match self {
            Self::I32(_) => MathType::scalar(MathDType::I32),
            Self::I64(_) => MathType::scalar(MathDType::I64),
            Self::U32(_) => MathType::scalar(MathDType::U32),
            Self::U64(_) => MathType::scalar(MathDType::U64),
            Self::F32(_) => MathType::scalar(MathDType::F32),
            Self::F64(_) => MathType::scalar(MathDType::F64),
            Self::Bool(_) => MathType::scalar(MathDType::Bool),
            Self::Bytes(_) => MathType::bytes(),
            Self::Tensor(t) => MathType { kind: MathTypeKind::Tensor, dtype: t.desc.dtype },
            Self::Unit => MathType::unit(),
        }
    }
}