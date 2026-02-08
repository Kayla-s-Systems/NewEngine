#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;

#[derive(Clone, Debug)]
pub struct ModelSourcePath(pub String);

#[derive(Clone, Copy, Debug)]
pub struct GridSettings {
    pub half_extent: f32,
    pub step: f32,
    pub major_step: f32,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            half_extent: 50.0,
            step: 1.0,
            major_step: 10.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GridTag;

#[derive(Clone, Copy, Debug, Default)]
pub struct ModelTag;

#[derive(Clone, Copy, Debug)]
pub struct Selected(pub EntityId);