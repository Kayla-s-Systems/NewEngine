#![forbid(unsafe_op_in_unsafe_fn)]

use std::fmt;

use crate::{Mat4, Quat, Vec2, Vec3, Vec4};

pub type MathResult<T> = Result<T, MathError>;

#[derive(Debug, Clone)]
pub enum MathError {
    NotFound { id: String },
    InvalidArgs { expected: Signature, got: Vec<MathValueType> },
    ProviderError { id: String, message: String },
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MathError::NotFound { id } => write!(f, "math function not found: {id}"),
            MathError::InvalidArgs { expected, got } => {
                write!(f, "invalid args: expected {expected:?}, got {got:?}")
            }
            MathError::ProviderError { id, message } => write!(f, "provider error ({id}): {message}"),
        }
    }
}

impl std::error::Error for MathError {}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathValueType {
    Unit = 0,
    Bool,
    I32,
    U32,
    F32,
    F64,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    Mat4,
    Bytes,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub inputs: Vec<MathValueType>,
    pub output: MathValueType,
}

#[derive(Debug, Clone)]
pub enum MathValue {
    Unit,
    Bool(bool),
    I32(i32),
    U32(u32),
    F32(f32),
    F64(f64),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
    Quat(Quat),
    Mat4(Mat4),
    Bytes(Vec<u8>),
    String(String),
}

impl MathValue {
    #[inline]
    pub fn ty(&self) -> MathValueType {
        match self {
            MathValue::Unit => MathValueType::Unit,
            MathValue::Bool(_) => MathValueType::Bool,
            MathValue::I32(_) => MathValueType::I32,
            MathValue::U32(_) => MathValueType::U32,
            MathValue::F32(_) => MathValueType::F32,
            MathValue::F64(_) => MathValueType::F64,
            MathValue::Vec2(_) => MathValueType::Vec2,
            MathValue::Vec3(_) => MathValueType::Vec3,
            MathValue::Vec4(_) => MathValueType::Vec4,
            MathValue::Quat(_) => MathValueType::Quat,
            MathValue::Mat4(_) => MathValueType::Mat4,
            MathValue::Bytes(_) => MathValueType::Bytes,
            MathValue::String(_) => MathValueType::String,
        }
    }
}
