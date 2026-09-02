#![forbid(unsafe_op_in_unsafe_fn)]

//! FPS content/policy bridge. Concrete items, loadouts, characters and mission text come from the active project policy.

pub mod authored_world_profile;
mod env_config;
mod mission;
mod world_runtime;

mod project_vfx;

pub use mission::{instantiate_authored_mission, AuthoredMissionSpawnSummary};

use std::{collections::BTreeSet, sync::Arc};

use newengine_engine_runtime::gameplay::{
    apply_loadout, GameplayContentProvider, GameplayWorld, InventoryLoadoutCatalog, ItemCatalog,
    ItemId, PlayerController, PlayerInventory,
};
use newengine_gameplay_fps_api::{
    FpsActorLoadoutRequest, FpsGameplayPolicyProvider, FpsGameplayPolicySnapshot, FpsRuntimeRules,
};
use newengine_gameplay_script_api::ScriptedStateMachineEventRequest;
use newengine_gameplay_script_runtime::{
    dispatch_state_machine_event, register_state_machine_instance, ScriptedStateMachineInstance,
    ScriptedStateMachineStore,
};

#[cfg(not(any(test, feature = "test-support")))]
use newengine_item_assets_runtime::decode_authored_item_package_nef8;
use newengine_item_assets_runtime::{
    compile_authored_item_package, hydrate_item_package_from_ytyp, install_compiled_item_package,
    AuthoredItemPackage,
};

#[cfg(not(any(test, feature = "test-support")))]
const SHARED_WEAPON_CATALOG_PATH: &str = "items/shared_weapons.neitems";

#[cfg_attr(feature = "test-support", allow(dead_code))]
fn merge_shared_weapon_package(
    project: &mut AuthoredItemPackage,
    shared: AuthoredItemPackage,
) -> Result<(usize, usize), String> {
    let mut shared_ids = BTreeSet::new();
    let mut shared_weapons = Vec::new();
    for item in shared.items {
        if !item.kind.trim().eq_ignore_ascii_case("weapon") {
            continue;
        }
        if !item.id.starts_with("weapon.") {
            return Err(format!(
                "Shared weapon catalog contains non-canonical weapon id '{}'",
                item.id
            ));
        }
        if !shared_ids.insert(item.id.clone()) {
            return Err(format!(
                "Shared weapon catalog contains duplicate weapon id '{}'",
                item.id
            ));
        }
        shared_weapons.push(item);
    }
    if shared_weapons.is_empty() {
        return Err("Shared weapon catalog contains no weapon definitions".to_owned());
    }

    let before = project.items.len();
    project.items.retain(|item| !shared_ids.contains(&item.id));
    let replaced = before.saturating_sub(project.items.len());
    let added = shared_weapons.len();
    project.items.extend(shared_weapons);
    Ok((replaced, added))
}

#[cfg(not(any(test, feature = "test-support")))]
fn load_shared_weapon_package() -> Result<AuthoredItemPackage, String> {
    let assets =
        newengine_assets_api::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let bytes = assets
        .raw_bytes_v1(SHARED_WEAPON_CATALOG_PATH)
        .map_err(|error| {
            format!(
                "Shared weapon catalog read failed path='{}': {error}",
                SHARED_WEAPON_CATALOG_PATH
            )
        })?;
    decode_authored_item_package_nef8(&bytes).map_err(|error| {
        format!(
            "Shared weapon catalog decode failed path='{}': {error}",
            SHARED_WEAPON_CATALOG_PATH
        )
    })
}

// Test fixture identities stay local to test-support. Production runtime has no concrete
// weapon/ammo/loadout identity constants.
#[cfg(any(test, feature = "test-support"))]
pub const DEFAULT_FPS_LOADOUT_NAME: &str = "loadout.fps.default";
#[cfg(any(test, feature = "test-support"))]
pub const DEFAULT_MEDKIT_ITEM_NAME: &str = "consumable.medkit.standard";
#[cfg(any(test, feature = "test-support"))]
pub const DEFAULT_RIFLE_AMMO_NAME: &str = "ammo.rifle.standard";
#[cfg(any(test, feature = "test-support"))]
pub const DEFAULT_RIFLE_ITEM_NAME: &str = "weapon.rifle.standard";

#[cfg(any(test, feature = "test-support"))]
#[inline]
pub fn default_rifle_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_ITEM_NAME).expect("valid FPS item name")
}

#[cfg(any(test, feature = "test-support"))]
#[inline]
pub fn default_rifle_ammo_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_AMMO_NAME).expect("valid FPS ammo name")
}

#[cfg(any(test, feature = "test-support"))]
#[inline]
pub fn default_medkit_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_MEDKIT_ITEM_NAME).expect("valid FPS item name")
}

#[cfg(any(test, feature = "test-support"))]
#[inline]
pub fn default_fps_loadout_id() -> ItemId {
    ItemId::from_name(DEFAULT_FPS_LOADOUT_NAME).expect("valid FPS loadout name")
}

/// Installs FPS inventory content produced by the active gameplay-policy provider.
/// The generic engine inventory remains the execution mechanism; Lua only authors
/// the data and policy snapshot. There is deliberately no embedded runtime fallback.
pub struct FpsContentProvider {
    policy_provider: Arc<dyn FpsGameplayPolicyProvider>,
}

impl FpsContentProvider {
    #[inline]
    pub fn shared(
        policy_provider: Arc<dyn FpsGameplayPolicyProvider>,
    ) -> Arc<dyn GameplayContentProvider> {
        Arc::new(Self { policy_provider })
    }

    fn load_compiled_content(
        &self,
    ) -> Result<
        (
            Arc<FpsGameplayPolicySnapshot>,
            newengine_item_assets_runtime::CompiledItemPackage,
        ),
        String,
    > {
        let policy = self.policy_provider.load_snapshot()?;
        policy.validate()?;
        let mut authored: AuthoredItemPackage = serde_json::from_value(policy.content.clone())
            .map_err(|error| format!("Lua FPS item package decode failed: {error}"))?;
        #[cfg(not(any(test, feature = "test-support")))]
        {
            let shared = load_shared_weapon_package()?;
            let (replaced, added) = merge_shared_weapon_package(&mut authored, shared)?;
            newengine_ulog_api::ulog::info!(
                "fps gameplay content merged Shared weapon catalog path='{}' weapons={} replaced_project_aliases={} policy='shared-weapon-authority'",
                SHARED_WEAPON_CATALOG_PATH,
                added,
                replaced,
            );
        }
        let hydrated = hydrate_item_package_from_ytyp(&mut authored)
            .map_err(|error| format!("FPS item YTYP hydration failed: {error}"))?;
        let compiled = compile_authored_item_package(&authored).map_err(|error| {
            format!("FPS item package compile failed after YTYP hydration: {error}")
        })?;
        if hydrated > 0 {
            newengine_ulog_api::ulog::info!(
                "fps gameplay content hydrated {} item definition(s) from engine.assets.definitions/.ytyp",
                hydrated
            );
        }
        validate_required_content(&policy, &compiled)?;
        Ok((policy, compiled))
    }
}

impl GameplayContentProvider for FpsContentProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "newengine.gameplay.fps.content.lua-policy"
    }

    fn install(&self, world: &mut GameplayWorld) -> Result<(), String> {
        let (policy, package) = self.load_compiled_content()?;
        // Presentation assets are an independent failure domain. A malformed project FXD must
        // never prevent installation of gameplay policy/event subscriptions, inventory content,
        // scripted state machines, or audio routing. We still fail the VFX capability explicitly:
        // no synthetic effect is substituted and the degraded state is logged.
        let vfx_error =
            project_vfx::install_project_vfx_dictionaries(world, &package.catalog).err();
        if vfx_error.is_some() {
            project_vfx::install_empty_project_vfx_resources(world);
        }
        install_compiled_item_package(world, package);
        install_policy_resources(world, policy.as_ref());
        ensure_scripted_mission_state_machine(world, policy.as_ref())?;
        if let Some(error) = vfx_error {
            newengine_ulog_api::ulog::warn!(
                "fps project VFX unavailable err='{}' policy='explicit-degraded-no-fallback; gameplay policy remains active'",
                error
            );
        }
        newengine_ulog_api::ulog::info!(
            "fps gameplay policy installed provider='{}' schema='{}' version={} items_source='lua structured content' default_loadout='{}' callbacks=[interaction:'{}',hit:'{}',mission:'{}']",
            self.policy_provider.id(),
            policy.schema,
            policy.version,
            policy.required_content.default_loadout,
            policy.callbacks.interaction,
            policy.callbacks.hit,
            policy.callbacks.mission_event,
        );
        Ok(())
    }

    fn content_is_present(&self, world: &GameplayWorld) -> bool {
        let Some(policy) = world.resource::<FpsGameplayPolicySnapshot>() else {
            return false;
        };
        let required = &policy.required_content;
        world.resource::<ItemCatalog>().is_some_and(|catalog| {
            catalog.find(&required.primary_weapon).is_some()
                && catalog.find(&required.primary_ammo).is_some()
                && catalog.find(&required.medkit).is_some()
        }) && world
            .resource::<InventoryLoadoutCatalog>()
            .is_some_and(|loadouts| {
                ItemId::from_name(&required.default_loadout)
                    .is_some_and(|id| loadouts.get(id).is_some())
            })
    }
}

fn ensure_scripted_mission_state_machine(
    world: &mut GameplayWorld,
    policy: &FpsGameplayPolicySnapshot,
) -> Result<(), String> {
    let authored = &policy.mission.state_machine;
    if !authored.enabled {
        return Ok(());
    }
    if world
        .resource::<ScriptedStateMachineStore>()
        .is_some_and(|store| store.get(&authored.instance_id).is_some())
    {
        return Ok(());
    }
    register_state_machine_instance(
        world,
        ScriptedStateMachineInstance::new(
            authored.instance_id.clone(),
            authored.machine_id.clone(),
            authored.initial_state.clone(),
        ),
    )?;
    dispatch_state_machine_event(
        world,
        ScriptedStateMachineEventRequest {
            instance_id: authored.instance_id.clone(),
            event: authored.activate_event.clone(),
            context: serde_json::Value::Null,
        },
    )?;
    Ok(())
}

fn validate_required_content(
    policy: &FpsGameplayPolicySnapshot,
    package: &newengine_item_assets_runtime::CompiledItemPackage,
) -> Result<(), String> {
    let required = &policy.required_content;
    for (label, id) in [
        ("primary_weapon", &required.primary_weapon),
        ("primary_ammo", &required.primary_ammo),
        ("medkit", &required.medkit),
    ] {
        if package.catalog.find(id).is_none() {
            return Err(format!(
                "Lua FPS item package is missing required {label} item '{id}'"
            ));
        }
    }
    let loadout = ItemId::from_name(&required.default_loadout).ok_or_else(|| {
        format!(
            "invalid Lua FPS default loadout id '{}'",
            required.default_loadout
        )
    })?;
    if package.loadouts.get(loadout).is_none() {
        return Err(format!(
            "Lua FPS item package is missing required loadout '{}'",
            required.default_loadout
        ));
    }
    Ok(())
}

fn install_policy_resources(world: &mut GameplayWorld, policy: &FpsGameplayPolicySnapshot) {
    world.insert_resource(policy.clone());
    let reconciled =
        newengine_fps_character_runtime::reconcile_existing_player_assignments_with_policy(
            world, policy,
        );
    if reconciled > 0 {
        newengine_ulog_api::ulog::info!(
            "fps gameplay policy reconciled character presentation assignments count={}",
            reconciled
        );
    }
    if let Some(rules) = world.resource_mut::<FpsRuntimeRules>() {
        let mission = &policy.mission;
        rules.default_status = mission.default_status.clone();
        rules.pickup_status = mission.pickup_status.clone();
        rules.target_status = mission.target_status.clone();
        rules.hazard_status = mission.hazard_status.clone();
        rules.goal_locked_status = mission.goal_locked_status.clone();
        rules.goal_complete_status = mission.goal_complete_status.clone();
        rules.failed_progress_label = mission.failed_progress_label.clone();
        rules.completed_progress_label = mission.completed_progress_label.clone();
    }
    if let Some(state) = world.resource_mut::<newengine_gameplay_fps_api::FpsObjectiveState>() {
        state.failed_progress_label = policy.mission.failed_progress_label.clone();
        state.completed_progress_label = policy.mission.completed_progress_label.clone();
        if !state.completed && !state.failed {
            state.status = policy.mission.default_status.clone();
        }
    }
}

pub fn ensure_fps_actor_loadouts(world: &mut GameplayWorld) {
    if world.resource::<ItemCatalog>().is_none()
        || world.resource::<InventoryLoadoutCatalog>().is_none()
    {
        return;
    }
    let requests = world
        .query::<FpsActorLoadoutRequest>()
        .map(|(entity, request)| (entity, request.clone()))
        .collect::<Vec<_>>();
    for (actor, request) in requests {
        let logical_name = request.loadout.trim();
        let Some(loadout) = ItemId::from_name(logical_name) else {
            newengine_ulog_api::ulog::error!(
                "FPS actor loadout rejected actor={} loadout='{}' reason='empty logical id'",
                actor.stable_u64(),
                request.loadout,
            );
            let _ = world.remove::<FpsActorLoadoutRequest>(actor);
            continue;
        };
        match apply_loadout(world, actor, loadout) {
            Ok(()) => {
                let _ = world.remove::<FpsActorLoadoutRequest>(actor);
            }
            Err(error) => {
                newengine_ulog_api::ulog::error!(
                    "FPS actor loadout rejected actor={} loadout='{}' err='{}' policy='no fallback loadout'",
                    actor.stable_u64(),
                    logical_name,
                    error,
                );
                let _ = world.remove::<FpsActorLoadoutRequest>(actor);
            }
        }
    }
}

pub fn ensure_fps_player_loadouts(world: &mut GameplayWorld) {
    if world.resource::<ItemCatalog>().is_none()
        || world.resource::<InventoryLoadoutCatalog>().is_none()
    {
        return;
    }
    let Some(default_loadout) = world
        .resource::<FpsGameplayPolicySnapshot>()
        .and_then(|policy| ItemId::from_name(&policy.required_content.default_loadout))
    else {
        return;
    };

    let players = world
        .query::<PlayerController>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for player in players {
        let Some(inventory) = world.get::<PlayerInventory>(player) else {
            continue;
        };
        if inventory.loadout_initialized() {
            continue;
        }
        if !inventory.entries.is_empty() {
            if let Some(inventory) = world.get_mut::<PlayerInventory>(player) {
                inventory.mark_loadout_initialized();
            }
            continue;
        }
        let _ = apply_loadout(world, player, default_loadout);
    }
}

#[cfg(any(test, feature = "test-support"))]
pub fn embedded_test_policy_provider() -> Arc<dyn FpsGameplayPolicyProvider> {
    Arc::new(EmbeddedTestPolicyProvider)
}

#[cfg(any(test, feature = "test-support"))]
pub fn embedded_test_content_provider() -> FpsContentProvider {
    FpsContentProvider {
        policy_provider: embedded_test_policy_provider(),
    }
}

#[cfg(any(test, feature = "test-support"))]
struct EmbeddedTestPolicyProvider;

#[cfg(any(test, feature = "test-support"))]
impl FpsGameplayPolicyProvider for EmbeddedTestPolicyProvider {
    fn id(&self) -> &'static str {
        "test.fps.embedded-policy"
    }

    fn load_snapshot(&self) -> Result<Arc<FpsGameplayPolicySnapshot>, String> {
        let authored = newengine_item_assets_runtime::test_fps_item_package();
        let policy = FpsGameplayPolicySnapshot {
            content: serde_json::to_value(authored)
                .map_err(|error| format!("test item package JSON encode failed: {error}"))?,
            required_content: newengine_gameplay_fps_api::FpsRequiredContentPolicy {
                default_loadout: DEFAULT_FPS_LOADOUT_NAME.to_owned(),
                primary_weapon: DEFAULT_RIFLE_ITEM_NAME.to_owned(),
                primary_ammo: DEFAULT_RIFLE_AMMO_NAME.to_owned(),
                medkit: DEFAULT_MEDKIT_ITEM_NAME.to_owned(),
            },
            mission: newengine_gameplay_fps_api::FpsMissionPolicy {
                default_status: "test mission".to_owned(),
                pickup_status: "test pickup".to_owned(),
                target_status: "test target".to_owned(),
                hazard_status: "test hazard".to_owned(),
                goal_locked_status: "test goal locked".to_owned(),
                goal_complete_status: "test goal complete".to_owned(),
                failed_progress_label: "test failed".to_owned(),
                completed_progress_label: "test completed".to_owned(),
                ..newengine_gameplay_fps_api::FpsMissionPolicy::default()
            },
            callbacks: newengine_gameplay_fps_api::FpsCallbackExports {
                interaction: "on_interaction".to_owned(),
                hit: "on_hit".to_owned(),
                mission_event: "on_mission_event".to_owned(),
            },
            ..FpsGameplayPolicySnapshot::default()
        };
        policy.validate()?;
        Ok(Arc::new(policy))
    }

    fn invoke_event(
        &self,
        _export: &str,
        _event: &newengine_gameplay_fps_api::FpsPolicyEvent,
    ) -> Result<newengine_gameplay_fps_api::FpsPolicyDecision, String> {
        Ok(newengine_gameplay_fps_api::FpsPolicyDecision::default())
    }
}

#[cfg(test)]
mod shared_weapon_catalog_tests {
    use super::*;
    use newengine_item_assets_runtime::AuthoredItemDefinition;

    fn item(id: &str, kind: &str, definition_ref: &str) -> AuthoredItemDefinition {
        AuthoredItemDefinition {
            id: id.to_owned(),
            kind: kind.to_owned(),
            definition_ref: definition_ref.to_owned(),
            ..AuthoredItemDefinition::default()
        }
    }

    #[test]
    fn authored_actor_loadout_request_applies_without_player_controller() {
        let mut world = GameplayWorld::new();
        let content = embedded_test_content_provider();
        GameplayContentProvider::install(&content, &mut world).expect("install FPS test content");
        let actor = world.spawn();
        let _ = world.insert(actor, FpsActorLoadoutRequest::new("loadout.fps.default"));

        ensure_fps_actor_loadouts(&mut world);

        assert!(world.get::<FpsActorLoadoutRequest>(actor).is_none());
        assert!(world.get::<PlayerController>(actor).is_none());
        assert!(world.get::<PlayerInventory>(actor).is_some());
        assert!(
            newengine_engine_runtime::gameplay::active_equipped_weapon_binding(&world, actor)
                .is_some(),
            "authored actor loadout must enter the same equipped-weapon inventory runtime"
        );
    }

    #[test]
    fn shared_weapon_catalog_adds_unlisted_weapon_and_replaces_project_alias() {
        let mut project = AuthoredItemPackage {
            items: vec![
                item(
                    "weapon.rifle.standard",
                    "weapon",
                    "project/forbidden.ytyp@rifle",
                ),
                item("consumable.medkit.standard", "consumable", ""),
            ],
            ..AuthoredItemPackage::default()
        };
        let shared = AuthoredItemPackage {
            items: vec![
                item(
                    "weapon.rifle.standard",
                    "weapon",
                    "shared/definitions/weapon/rifle.ytyp@rifle",
                ),
                item(
                    "weapon.rifle.mini14",
                    "weapon",
                    "shared/definitions/weapon/rifle_mini14.ytyp@rifle_mini14",
                ),
                item("ammo.rifle.standard", "ammo", ""),
            ],
            ..AuthoredItemPackage::default()
        };

        let (replaced, added) = merge_shared_weapon_package(&mut project, shared).expect("merge");
        assert_eq!(replaced, 1);
        assert_eq!(added, 2);
        assert!(project
            .items
            .iter()
            .any(|item| item.id == "weapon.rifle.mini14"));
        assert!(project.items.iter().any(|item| {
            item.id == "weapon.rifle.standard"
                && item.definition_ref == "shared/definitions/weapon/rifle.ytyp@rifle"
        }));
        assert!(project
            .items
            .iter()
            .any(|item| item.id == "consumable.medkit.standard"));
        assert_eq!(
            project
                .items
                .iter()
                .filter(|item| item.id == "weapon.rifle.standard")
                .count(),
            1
        );
    }
}

pub use world_runtime::{
    install_fps_content_world_runtime, install_fps_content_world_runtime_adapter,
    FpsContentWorldRuntimeAdapter, FpsContentWorldRuntimeBinding, FpsContentWorldRuntimeProvider,
};
