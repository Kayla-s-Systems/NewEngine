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

#[derive(Clone, Copy, Debug, Default)]
struct PresentationSubstageTiming {
    model_assignments_ms: f32,
    model_grounding_ms: f32,
    weapon_input_ms: f32,
    semantic_capture_ms: f32,
    camera_anchors_ms: f32,
    skin_animation_ms: f32,
    skin_sidecars_ms: f32,
    weapon_visuals_ms: f32,
    shell_casings_ms: f32,
    impact_debris_ms: f32,
    weapon_animation_ms: f32,
    decal_materials_ms: f32,
}

fn emit_presentation_profile(
    frame_index: u64,
    total_ms: f32,
    substages: Option<PresentationSubstageTiming>,
) {
    if !should_emit_presentation_profile(frame_index, total_ms) && substages.is_none() {
        return;
    }
    let substages = substages.unwrap_or_default();
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
        "model_assignments_ms": substages.model_assignments_ms,
        "model_grounding_ms": substages.model_grounding_ms,
        "weapon_input_ms": substages.weapon_input_ms,
        "semantic_capture_ms": substages.semantic_capture_ms,
        "camera_anchors_ms": substages.camera_anchors_ms,
        "skin_animation_ms": substages.skin_animation_ms,
        "skin_sidecars_ms": substages.skin_sidecars_ms,
        "weapon_visuals_ms": substages.weapon_visuals_ms,
        "shell_casings_ms": substages.shell_casings_ms,
        "impact_debris_ms": substages.impact_debris_ms,
        "weapon_animation_ms": substages.weapon_animation_ms,
        "decal_materials_ms": substages.decal_materials_ms,
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
        let detail_sample = frame.frame_index.is_multiple_of(30);
        let mut timing = PresentationSubstageTiming::default();

        let phase = detail_sample.then(std::time::Instant::now);
        crate::player_model::tick_player_model_assignments(world, primitives, materials);
        if let Some(phase) = phase {
            timing.model_assignments_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::player_model::tick_player_model_grounding(world);
        if let Some(phase) = phase {
            timing.model_grounding_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::equipment_visual::tick_equipped_weapon_presentation_input(world, frame.dt);
        if let Some(phase) = phase {
            timing.weapon_input_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::animation_semantic::capture_animation_semantic_frame(world);
        if let Some(phase) = phase {
            timing.semantic_capture_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::player_model::publish_player_first_person_camera_anchors(world);
        if let Some(phase) = phase {
            timing.camera_anchors_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::player_model::tick_player_skin_animation(world, frame.dt, frame.frame_index);
        if let Some(phase) = phase {
            timing.skin_animation_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::player_model::tick_player_skin_sidecars(world);
        if let Some(phase) = phase {
            timing.skin_sidecars_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::equipment_visual::tick_equipped_weapon_visuals(
            world, primitives, materials, frame.dt,
        );
        if let Some(phase) = phase {
            timing.weapon_visuals_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::weapon_casing::tick_weapon_shell_casing_visuals(world, primitives, materials);
        if let Some(phase) = phase {
            timing.shell_casings_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::impact_debris::tick_persistent_impact_debris_visuals(world, primitives, materials);
        if let Some(phase) = phase {
            timing.impact_debris_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::weapon_animation::tick_equipped_weapon_animations(world, frame.dt);
        if let Some(phase) = phase {
            timing.weapon_animation_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        let phase = detail_sample.then(std::time::Instant::now);
        crate::vfx_decal_materials::tick_vfx_decal_material_bindings(world, materials);
        if let Some(phase) = phase {
            timing.decal_materials_ms = phase.elapsed().as_secs_f32() * 1000.0;
        }

        emit_presentation_profile(
            frame.frame_index,
            started.elapsed().as_secs_f32() * 1000.0,
            detail_sample.then_some(timing),
        );
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
