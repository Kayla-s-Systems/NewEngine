use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamepadAxisTarget {
    MoveX,
    MoveY,
    MoveZ,
    LookX,
    LookY,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GamepadAxisBinding {
    pub axis: String,
    pub target: GamepadAxisTarget,
    #[serde(default = "default_axis_deadzone")]
    pub deadzone: f32,
    #[serde(default = "default_axis_scale")]
    pub scale: f32,
}

#[inline]
fn default_axis_deadzone() -> f32 {
    0.18
}
#[inline]
fn default_axis_scale() -> f32 {
    1.0
}

impl GamepadAxisBinding {
    #[inline]
    pub fn new(axis: impl Into<String>, target: GamepadAxisTarget, scale: f32) -> Self {
        Self {
            axis: axis.into(),
            target,
            deadzone: default_axis_deadzone(),
            scale,
        }
    }
}
