use newengine_engine_runtime::gameplay::WeaponPresentationDefinition;
use newengine_math::{Mat3, Mat4, Quat, Vec3};

include!("weapon_grip/anchors.rs");
include!("weapon_grip/camera.rs");
include!("weapon_grip/solve.rs");
include!("weapon_grip/constraints.rs");

#[cfg(test)]
#[path = "weapon_grip/tests.rs"]
mod tests;
