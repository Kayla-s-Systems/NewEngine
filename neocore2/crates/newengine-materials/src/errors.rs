#![allow(dead_code)]

use core::fmt;

pub type MaterialResult<T> = Result<T, MaterialError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialError {
    InvalidId,
    AlreadyExists,
    NotFound,
}

impl fmt::Display for MaterialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaterialError::InvalidId => write!(f, "invalid material id"),
            MaterialError::AlreadyExists => write!(f, "material already exists"),
            MaterialError::NotFound => write!(f, "material not found"),
        }
    }
}

impl std::error::Error for MaterialError {}
