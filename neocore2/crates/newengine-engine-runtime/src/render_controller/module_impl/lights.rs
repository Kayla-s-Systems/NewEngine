#![forbid(unsafe_op_in_unsafe_fn)]

pub(crate) use newengine_render_feature_api::{
    primary_directional_light, PackedLights,
};

#[inline]
pub(super) fn collect_lights(world: &newengine_ecs::World) -> PackedLights {
    PackedLights::from_world(world)
}
