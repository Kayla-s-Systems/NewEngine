use newengine_tags_api::TagId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct NavVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl NavVec3 {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavPathPointV1 {
    pub position: NavVec3,
    #[serde(default)]
    pub flags: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NavPathDtoV1 {
    #[serde(default)]
    pub points: Vec<NavPathPointV1>,
    #[serde(default)]
    pub cost: f32,
    #[serde(default)]
    pub complete: bool,
}
