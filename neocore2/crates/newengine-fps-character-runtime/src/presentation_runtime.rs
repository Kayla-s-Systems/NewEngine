#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::ThreadPoolHandle;
use newengine_ecs::World;
use newengine_materials::MaterialRegistry;
use newengine_primitives::PrimitiveRegistry;
use newengine_world_runtime_api::{WorldRuntimeFrame, WorldRuntimeProvider};

pub trait FpsCharacterPresentationRuntimeAdapter: Send + Sync {
    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    );
}

#[derive(Clone)]
pub struct FpsCharacterPresentationRuntimeBinding(
    pub Arc<dyn FpsCharacterPresentationRuntimeAdapter>,
);

pub fn install_fps_character_presentation_runtime_adapter(
    world: &mut World,
    adapter: Arc<dyn FpsCharacterPresentationRuntimeAdapter>,
) {
    world.insert_resource(FpsCharacterPresentationRuntimeBinding(adapter));
}

#[inline]
fn should_emit_presentation_profile(frame_index: u64, total_ms: f32) -> bool {
    total_ms >= 4.0 || frame_index.is_multiple_of(120)
}

fn emit_presentation_profile(frame_index: u64, total_ms: f32) {
    if !should_emit_presentation_profile(frame_index, total_ms) {
        return;
    }
    let payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "world.runtime.contribution",
        "source": "newengine-fps-character-runtime",
        "name": "engine.fps.character-presentation",
        "lane": "world-runtime",
        "priority": "interactive",
        "dependency_group": format!("world.runtime.frame.{frame_index}"),
        "frame_index": frame_index,
        "elapsed_ms": total_ms,
        "budget_ms": 4.0,
        "slow": total_ms >= 4.0,
    });
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        let _ = newengine_plugin_host::emit_plugin_event(
            "newengine.diagnostics.profiler.sample.v1",
            &bytes,
        );
    }
}

struct FpsCharacterPresentationRuntime;

impl FpsCharacterPresentationRuntimeAdapter for FpsCharacterPresentationRuntime {
    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        _thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let started = std::time::Instant::now();
        crate::player_model::tick_player_model_assignments(world, primitives, materials);
        crate::player_model::tick_player_model_grounding(world);
        crate::equipment_visual::tick_equipped_weapon_presentation_input(world, frame.dt);
        crate::animation_semantic::capture_animation_semantic_frame(world);
        crate::player_model::publish_player_first_person_camera_anchors(world);
        crate::player_model::tick_player_skin_animation(world, frame.dt, frame.frame_index);
        crate::player_model::tick_player_skin_sidecars(world);
        crate::equipment_visual::tick_equipped_weapon_visuals(
            world, primitives, materials, frame.dt,
        );
        crate::weapon_casing::tick_weapon_shell_casing_visuals(world, primitives, materials);
        crate::impact_debris::tick_persistent_impact_debris_visuals(world, primitives, materials);
        crate::weapon_animation::tick_equipped_weapon_animations(world, frame.dt);
        crate::vfx_decal_materials::tick_vfx_decal_material_bindings(world, materials);
        emit_presentation_profile(frame.frame_index, started.elapsed().as_secs_f32() * 1000.0);
    }
}

/// Install the domain-owned concrete FPS character presentation implementation.
pub fn install_fps_character_presentation_runtime(world: &mut World) {
    install_fps_character_presentation_runtime_adapter(
        world,
        Arc::new(FpsCharacterPresentationRuntime),
    );
}

pub struct FpsCharacterPresentationWorldRuntimeProvider;

impl FpsCharacterPresentationWorldRuntimeProvider {
    #[inline]
    pub fn shared() -> Arc<dyn WorldRuntimeProvider> {
        Arc::new(Self)
    }
}

impl WorldRuntimeProvider for FpsCharacterPresentationWorldRuntimeProvider {
    fn id(&self) -> &'static str {
        "engine.fps.character-presentation"
    }

    fn tick_frame(
        &self,
        world: &mut World,
        primitives: &mut PrimitiveRegistry,
        materials: &MaterialRegistry,
        thread_pool: Option<&ThreadPoolHandle>,
        frame: WorldRuntimeFrame,
    ) {
        let binding = world
            .resource::<FpsCharacterPresentationRuntimeBinding>()
            .cloned();
        if let Some(binding) = binding {
            binding
                .0
                .tick_frame(world, primitives, materials, thread_pool, frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_identity_is_domain_owned() {
        assert_eq!(
            FpsCharacterPresentationWorldRuntimeProvider.id(),
            "engine.fps.character-presentation"
        );
    }
}
