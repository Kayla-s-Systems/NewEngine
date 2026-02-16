#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeTag {
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
    Unit,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MathValue {
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
    Unit,
}

impl MathValue {
    #[inline]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::F32(v) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn type_tag(&self) -> TypeTag {
        match self {
            Self::Bool(_) => TypeTag::Bool,
            Self::I32(_) => TypeTag::I32,
            Self::U32(_) => TypeTag::U32,
            Self::F32(_) => TypeTag::F32,
            Self::F64(_) => TypeTag::F64,
            Self::Vec2(_) => TypeTag::Vec2,
            Self::Vec3(_) => TypeTag::Vec3,
            Self::Vec4(_) => TypeTag::Vec4,
            Self::Quat(_) => TypeTag::Quat,
            Self::Mat4(_) => TypeTag::Mat4,
            Self::Unit => TypeTag::Unit,
        }
    }
}
