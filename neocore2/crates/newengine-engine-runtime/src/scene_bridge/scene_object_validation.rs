use super::*;
use crate::gameplay::{ModelRenderComponent, PreparedRenderMesh};
use newengine_bounds::Bounds;
use newengine_math::collections::FxHashMap;
use newengine_ui_api::UiStatePatch;
use std::sync::OnceLock;

const SCENE_OBJECT_INVARIANTS_SURFACE_ID: &str = "engine.ui.editor.scene_object_invariants";
const SCENE_OBJECT_INVARIANTS_SOURCE_GATEWAY: &str = "engine.scene";
const SCENE_OBJECT_INVARIANTS_CONTRACT: &str = "newengine.scene.object_invariants.snapshot.v1";
const SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID: &str = "invariants";

#[derive(Clone, Debug, Default)]
pub(crate) struct SceneObjectInvariantRepairRecord {
    pub entity: String,
    pub entity_key: u64,
    pub reasons: Vec<String>,
    pub repaired_transform: bool,
    pub repaired_bounds: bool,
    pub repaired_physics: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SceneObjectInvariantReport {
    pub checked: usize,
    pub repaired: usize,
    pub missing_transform: usize,
    pub missing_bounds: usize,
    pub missing_physics: usize,
    pub last_repaired_entities: Vec<SceneObjectInvariantRepairRecord>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub(crate) struct SceneObjectInvariantRuntimeDiagnostics {
    pub phase: String,
    pub last_report: SceneObjectInvariantReport,
}

#[derive(Clone, Debug)]
struct SceneObjectTarget {
    entity: EntityId,
    reasons: Vec<&'static str>,
}

pub(crate) fn validate_scene_object_invariants(
    world: &mut newengine_ecs::World,
    phase: &'static str,
) -> SceneObjectInvariantReport {
    let mut targets = Vec::<SceneObjectTarget>::new();
    let mut target_indices = FxHashMap::<u64, usize>::default();

    for (entity, _) in world.query::<Primitive>() {
        if world
            .get::<crate::gameplay::WorldItemVisualPart>(entity)
            .is_none()
        {
            add_target(&mut targets, &mut target_indices, entity, "Primitive");
        }
    }
    for (entity, _) in world.query::<ModelRenderComponent>() {
        add_target(
            &mut targets,
            &mut target_indices,
            entity,
            "ModelRenderComponent",
        );
    }
    for (entity, _) in world.query::<PreparedRenderMesh>() {
        add_target(
            &mut targets,
            &mut target_indices,
            entity,
            "PreparedRenderMesh",
        );
    }
    for (entity, _) in world.query::<crate::scene_bridge::definitions_runtime::DefinitionInstance>()
    {
        add_target(
            &mut targets,
            &mut target_indices,
            entity,
            "DefinitionInstance",
        );
    }
    for (entity, _) in world.query::<crate::gameplay::PlayerVisualPart>() {
        add_target(
            &mut targets,
            &mut target_indices,
            entity,
            "PlayerVisualPart",
        );
    }
    for (entity, anchor) in world.query::<crate::gameplay::SceneEntityAnchor>() {
        add_target_by_anchor(&mut targets, &mut target_indices, entity, anchor.role);
    }

    let mut report = SceneObjectInvariantReport {
        checked: targets.len(),
        ..SceneObjectInvariantReport::default()
    };

    for target in targets {
        let missing_transform = world.get::<Transform>(target.entity).is_none();
        let missing_bounds = world.get::<Bounds>(target.entity).is_none();
        // Physics is deliberately not a scene-object invariant. Render/model/terrain/player
        // presentation entities may be non-colliding; collision is authored explicitly through
        // PhysicsBodyDesc / StaticMeshCollider / terrain collision providers.
        let missing_physics = false;

        if !(missing_transform || missing_bounds) {
            continue;
        }

        report.repaired = report.repaired.saturating_add(1);
        report.missing_transform += usize::from(missing_transform);
        report.missing_bounds += usize::from(missing_bounds);
        report.missing_physics += usize::from(missing_physics);
        report
            .last_repaired_entities
            .push(SceneObjectInvariantRepairRecord {
                entity: format!("{:?}", target.entity),
                entity_key: target.entity.stable_u64(),
                reasons: target
                    .reasons
                    .iter()
                    .map(|reason| (*reason).to_owned())
                    .collect(),
                repaired_transform: missing_transform,
                repaired_bounds: missing_bounds,
                repaired_physics: missing_physics,
            });

        let position = world
            .get::<Transform>(target.entity)
            .map(|transform| transform.position)
            .unwrap_or(Vec3::ZERO);
        let half_extents = world
            .get::<Bounds>(target.entity)
            .map(|bounds| bounds.local_aabb.half_extents())
            .unwrap_or_else(|| Vec3::splat(0.25));

        crate::gameplay::attach_scene_object_core(world, target.entity, position, half_extents);
        newengine_ulog_api::ulog::warn!(
            "scene object invariant: phase='{}' entity={:?} reasons='{}' repaired_transform={} repaired_bounds={} repaired_physics={} policy='scene objects require Transform+Bounds; physics is explicit opt-in'",
            phase,
            target.entity,
            target.reasons.join(","),
            missing_transform,
            missing_bounds,
            missing_physics
        );
    }

    if report.repaired == 0 {
        newengine_ulog_api::ulog::debug!(
            "scene object invariants: phase='{}' status='stable' checked={} policy='render/model/terrain/definition/player scene objects are complete entities'",
            phase,
            report.checked
        );
    } else {
        newengine_ulog_api::ulog::warn!(
            "scene object invariants: phase='{}' status='repaired' checked={} repaired={} missing_transform={} missing_bounds={} missing_physics={} policy='new scene objects cannot remain incomplete'",
            phase,
            report.checked,
            report.repaired,
            report.missing_transform,
            report.missing_bounds,
            report.missing_physics
        );
    }

    world.insert_resource(SceneObjectInvariantRuntimeDiagnostics {
        phase: phase.to_owned(),
        last_report: report.clone(),
    });
    if scene_object_invariants_ui_enabled() {
        publish_scene_object_invariants_state_patch(phase, &report);
    }
    report
}

fn scene_object_invariants_ui_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        const OVERRIDE_ENV: &str = "NEWENGINE_SCENE_OBJECT_INVARIANTS_UI";
        const EDITOR_SHELL_ENV: &str =
            "NEWENGINE_PLUGIN_ENGINE_RUNTIME__ui__screen_profile__publish_editor_shell";

        if let Some(value) = crate::env_config::var(OVERRIDE_ENV) {
            return matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }

        let editor_shell_enabled = crate::env_config::var(EDITOR_SHELL_ENV)
            .map(|value| {
                !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                )
            })
            .unwrap_or(true);
        let runtime_target =
            crate::env_config::var("NEWENGINE_PLUGIN_TARGET").is_some_and(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "runtime" | "game" | "standalone"
                )
            });

        editor_shell_enabled && !runtime_target
    })
}

fn publish_scene_object_invariants_state_patch(
    phase: &'static str,
    report: &SceneObjectInvariantReport,
) {
    let repaired_entities = report
        .last_repaired_entities
        .iter()
        .map(|record| {
            serde_json::json!({
                "entity": record.entity,
                "reasons": record.reasons,
                "repaired_transform": record.repaired_transform,
                "repaired_bounds": record.repaired_bounds,
                "repaired_physics": record.repaired_physics,
            })
        })
        .collect::<Vec<_>>();
    let patch = UiStatePatch::new(0, SCENE_OBJECT_INVARIANTS_SURFACE_ID)
        .with_change(
            SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID,
            "phase",
            serde_json::json!(phase),
        )
        .with_change(
            SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID,
            "checked",
            serde_json::json!(report.checked),
        )
        .with_change(
            SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID,
            "repaired",
            serde_json::json!(report.repaired),
        )
        .with_change(
            SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID,
            "missing_transform",
            serde_json::json!(report.missing_transform),
        )
        .with_change(
            SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID,
            "missing_bounds",
            serde_json::json!(report.missing_bounds),
        )
        .with_change(
            SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID,
            "missing_physics",
            serde_json::json!(report.missing_physics),
        )
        .with_change(
            SCENE_OBJECT_INVARIANTS_STATE_SOURCE_ID,
            "last_repaired_entities",
            serde_json::Value::Array(repaired_entities),
        );
    crate::ui_gateway::publish_state_patch(
        &patch,
        SCENE_OBJECT_INVARIANTS_SOURCE_GATEWAY,
        SCENE_OBJECT_INVARIANTS_CONTRACT,
    );
}

pub(crate) fn scene_object_invariant_snapshot_json(
    world: &newengine_ecs::World,
) -> serde_json::Value {
    let diagnostics = world.resource::<SceneObjectInvariantRuntimeDiagnostics>();
    let phase = diagnostics
        .map(|diagnostics| diagnostics.phase.as_str())
        .unwrap_or("not-yet-run");
    let report = diagnostics
        .map(|diagnostics| diagnostics.last_report.clone())
        .unwrap_or_default();
    serde_json::json!({
        "ok": true,
        "schema": "newengine.scene.object_invariants.snapshot.v1",
        "phase": phase,
        "checked": report.checked,
        "repaired": report.repaired,
        "missing_transform": report.missing_transform,
        "missing_bounds": report.missing_bounds,
        "missing_physics": report.missing_physics,
        "last_repaired_entities": report.last_repaired_entities.iter().map(|record| serde_json::json!({
            "entity": record.entity,
            "entity_key": record.entity_key,
            "reasons": record.reasons,
            "repaired_transform": record.repaired_transform,
            "repaired_bounds": record.repaired_bounds,
            "repaired_physics": record.repaired_physics,
        })).collect::<Vec<_>>(),
        "policy": "scene objects require Transform + Bounds; physics is explicit opt-in",
    })
}

fn add_target(
    targets: &mut Vec<SceneObjectTarget>,
    target_indices: &mut FxHashMap<u64, usize>,
    entity: EntityId,
    reason: &'static str,
) {
    let key = entity.stable_u64();
    if let Some(&index) = target_indices.get(&key) {
        let existing = &mut targets[index];
        if !existing.reasons.contains(&reason) {
            existing.reasons.push(reason);
        }
        return;
    }
    let index = targets.len();
    targets.push(SceneObjectTarget {
        entity,
        reasons: vec![reason],
    });
    target_indices.insert(key, index);
}

fn add_target_by_anchor(
    targets: &mut Vec<SceneObjectTarget>,
    target_indices: &mut FxHashMap<u64, usize>,
    entity: EntityId,
    role: crate::gameplay::SceneEntityRole,
) {
    use crate::gameplay::SceneEntityRole;
    let reason = match role {
        SceneEntityRole::Environment => "SceneEntityAnchor::Environment",
        SceneEntityRole::Sun => "SceneEntityAnchor::Sun",
        SceneEntityRole::SkyCycle => "SceneEntityAnchor::SkyCycle",
        SceneEntityRole::SkyDome => "SceneEntityAnchor::SkyDome",
        SceneEntityRole::Terrain => "SceneEntityAnchor::Terrain",
        SceneEntityRole::TerrainStreamingAnchor => "SceneEntityAnchor::TerrainStreamingAnchor",
        SceneEntityRole::Foliage => "SceneEntityAnchor::Foliage",
        SceneEntityRole::Definitions => "SceneEntityAnchor::Definitions",
        SceneEntityRole::Actors => "SceneEntityAnchor::Actors",
        SceneEntityRole::Cameras => "SceneEntityAnchor::Cameras",
        SceneEntityRole::Player => "SceneEntityAnchor::Player",
        SceneEntityRole::ActiveCamera => "SceneEntityAnchor::ActiveCamera",
    };
    add_target(targets, target_indices, entity, reason);
}
