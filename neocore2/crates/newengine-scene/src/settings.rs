#![forbid(unsafe_op_in_unsafe_fn)]

/// Coordinate system definition for the scene.
///
/// Conventions (recommended engine-default):
/// - right-handed
/// - +Y up
/// - -Z forward
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpAxis {
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForwardAxis {
    NegZ,
    PosZ,
    PosX,
    NegX,
}

/// Scene unit scale: how many meters are in one world unit.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct UnitScaleMeters(pub f32);

impl Default for UnitScaleMeters {
    #[inline]
    fn default() -> Self {
        Self(1.0)
    }
}

/// Global scene settings (renderer-agnostic, editor-agnostic).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SceneSettings {
    pub up: UpAxis,
    pub forward: ForwardAxis,
    pub unit_scale_m: UnitScaleMeters,
}

impl Default for SceneSettings {
    #[inline]
    fn default() -> Self {
        Self {
            up: UpAxis::Y,
            forward: ForwardAxis::NegZ,
            unit_scale_m: UnitScaleMeters::default(),
        }
    }
}
