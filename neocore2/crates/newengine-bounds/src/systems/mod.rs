//! ECS systems for keeping derived bounds up to date.

mod update_from_mat4;

pub use update_from_mat4::update_bounds_from_mat4_system;

#[cfg(feature = "transform")]
mod update_from_transform;

#[cfg(feature = "transform")]
pub use update_from_transform::update_bounds_from_transform_system;
