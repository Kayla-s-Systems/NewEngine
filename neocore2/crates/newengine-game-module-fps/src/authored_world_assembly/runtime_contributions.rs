#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::World;

/// Installs domain-owned concrete runtime implementations.
/// FPS authored no longer owns character, authored-world, environment or content adapters.
pub(crate) fn install_world_runtime_adapters(world: &mut World) {
    newengine_fps_character_runtime::install_fps_character_presentation_runtime(world);
    newengine_authored_world_runtime::install_default_authored_world_streaming_runtime_adapter(
        world,
    );
    newengine_world_environment_runtime::install_authored_environment_runtime_adapter(world);
    newengine_fps_content_runtime::install_fps_content_world_runtime(world);
}
