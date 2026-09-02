use newengine_bounds::Bounds;
use newengine_ecs::EntityId;
use newengine_engine_runtime::world_authoring::{
    spawn_primitive as spawn_game_primitive, PrimitiveSpawnSpec,
};
use newengine_gameplay_fps_api::{
    FpsObjectiveGoal, FpsObjectiveHazard, FpsObjectivePickup, FpsObjectiveTarget,
};
use newengine_material_domain_api::AuthoredMaterialSpec;
use newengine_material_runtime::authored_registration::{
    register_required_material, register_required_material_ref,
};
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_model_runtime::ydd_runtime::{
    decode_runtime_ydd_prefab, DecodedRuntimeYddMeshPart as DecodedPrefabMeshPart,
};
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_transform::Transform;
use newengine_world_environment_runtime::authored_foliage::terrain_height;

use crate::authored_world_profile::{
    AuthoredMissionPickupSpec, AuthoredMissionSpec, AuthoredMissionTargetSpec,
};

const MISSION_STREAMING_PIN_OWNER: &str = "fps.content.mission";

#[derive(Debug, Default)]
struct MissionAssetPinSet {
    leases: std::collections::BTreeMap<String, newengine_assets::AssetStreamingPinLease>,
}

fn pin_mission_asset(world: &mut newengine_ecs::World, logical_path: &str) -> Result<(), String> {
    let logical_path = logical_path.trim().replace('\\', "/");
    if logical_path.is_empty() {
        return Ok(());
    }

    let mut pins = world
        .remove_resource::<MissionAssetPinSet>()
        .unwrap_or_default();
    if pins.leases.contains_key(&logical_path) {
        world.insert_resource(pins);
        return Ok(());
    }

    let client =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    match newengine_assets::AssetStreamingPinLease::acquire(
        client,
        logical_path.clone(),
        MISSION_STREAMING_PIN_OWNER,
        newengine_assets::AssetStreamingPinClassV1::Mission,
    ) {
        Ok(lease) => {
            pins.leases.insert(logical_path, lease);
            world.insert_resource(pins);
            Ok(())
        }
        Err(error) => {
            world.insert_resource(pins);
            Err(error)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredMissionSpawnSummary {
    /// Mission-objective pickups only (relay cores, etc.).
    pub pickups: u32,
    /// Inventory-backed authored world pickups. These never affect mission-core totals.
    pub item_pickups: u32,
    pub targets: u32,
    pub hazards: u32,
    pub goals: u32,
}

#[derive(Clone, Debug)]
struct DeferredWorldItemPickup {
    parent: EntityId,
    terrain: EntityId,
    spec: AuthoredMissionPickupSpec,
    attempts: u32,
}

#[derive(Clone, Debug, Default)]
struct DeferredWorldItemPickups {
    pending: Vec<DeferredWorldItemPickup>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RuntimeWorldItemAdmissionState {
    attempts: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct MissionMaterials {
    core: Option<MaterialId>,
    target: Option<MaterialId>,
    hazard: Option<MaterialId>,
    goal: Option<MaterialId>,
}

fn mission_material(
    mats: &MaterialRegistry,
    role: &str,
    authored_ref: Option<&str>,
    required: bool,
) -> Result<Option<MaterialId>, String> {
    if !required {
        return Ok(None);
    }
    let reference = authored_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "mission role '{}' requires a project-authored material reference",
                role
            )
        })?;
    let id = register_required_material_ref(
        mats,
        &format!("Mission/{role}"),
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
        reference,
    )?;
    Ok(Some(id))
}

fn register_mission_materials(
    mats: &MaterialRegistry,
    mission: &AuthoredMissionSpec,
) -> Result<MissionMaterials, String> {
    Ok(MissionMaterials {
        core: mission_material(
            mats,
            "Core",
            mission.core_material.as_deref(),
            !mission.pickups.is_empty(),
        )?,
        target: mission_material(
            mats,
            "Target",
            mission.target_material.as_deref(),
            !mission.targets.is_empty(),
        )?,
        hazard: mission_material(
            mats,
            "Hazard",
            mission.hazard_material.as_deref(),
            !mission.hazards.is_empty(),
        )?,
        goal: mission_material(
            mats,
            "Goal",
            mission.goal_material.as_deref(),
            !mission.goals.is_empty(),
        )?,
    })
}
