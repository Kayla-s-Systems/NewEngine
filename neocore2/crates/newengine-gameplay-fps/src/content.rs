use std::sync::Arc;

use newengine_engine_runtime::gameplay::{
    apply_loadout, GameplayContentProvider, GameplayWorld, InventoryLoadoutCatalog, ItemCatalog,
    ItemId, PlayerController, PlayerInventory,
};
use newengine_gameplay_fps_api::{
    FpsDemoRules, FpsGameplayPolicyProvider, FpsGameplayPolicySnapshot,
};
use newengine_gameplay_script_api::ScriptedStateMachineEventRequest;
use newengine_gameplay_script_runtime::{
    dispatch_state_machine_event, register_state_machine_instance, ScriptedStateMachineInstance,
    ScriptedStateMachineStore,
};

use crate::item_assets::{
    compile_authored_item_package, hydrate_item_package_from_ytyp, install_compiled_item_package,
    AuthoredItemPackage,
};

pub use newengine_game_data::{
    DEFAULT_FPS_LOADOUT_NAME, DEFAULT_MEDKIT_ITEM_NAME, DEFAULT_RIFLE_AMMO_NAME,
    DEFAULT_RIFLE_ITEM_NAME,
};

#[inline]
pub fn default_rifle_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_ITEM_NAME).expect("valid FPS item name")
}

#[inline]
pub fn default_rifle_ammo_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_AMMO_NAME).expect("valid FPS ammo name")
}

#[inline]
pub fn default_medkit_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_MEDKIT_ITEM_NAME).expect("valid FPS item name")
}

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
            crate::item_assets::CompiledItemPackage,
        ),
        String,
    > {
        let policy = self.policy_provider.load_snapshot()?;
        policy.validate()?;
        let mut authored: AuthoredItemPackage = serde_json::from_value(policy.content.clone())
            .map_err(|error| format!("Lua FPS item package decode failed: {error}"))?;
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
        install_compiled_item_package(world, package);
        install_policy_resources(world, policy.as_ref());
        ensure_scripted_mission_state_machine(world, policy.as_ref())?;
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
    package: &crate::item_assets::CompiledItemPackage,
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
    if let Some(rules) = world.resource_mut::<FpsDemoRules>() {
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
    if let Some(state) = world.resource_mut::<newengine_gameplay_fps_api::FpsDemoState>() {
        state.failed_progress_label = policy.mission.failed_progress_label.clone();
        state.completed_progress_label = policy.mission.completed_progress_label.clone();
        if !state.completed && !state.failed {
            state.status = policy.mission.default_status.clone();
        }
    }
}

pub(crate) fn ensure_fps_player_loadouts(world: &mut GameplayWorld) {
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

#[cfg(test)]
pub(crate) fn embedded_test_policy_provider() -> Arc<dyn FpsGameplayPolicyProvider> {
    Arc::new(EmbeddedTestPolicyProvider)
}

#[cfg(test)]
pub(crate) fn embedded_test_content_provider() -> FpsContentProvider {
    FpsContentProvider {
        policy_provider: embedded_test_policy_provider(),
    }
}

#[cfg(test)]
struct EmbeddedTestPolicyProvider;

#[cfg(test)]
impl FpsGameplayPolicyProvider for EmbeddedTestPolicyProvider {
    fn id(&self) -> &'static str {
        "test.fps.embedded-policy"
    }

    fn load_snapshot(&self) -> Result<Arc<FpsGameplayPolicySnapshot>, String> {
        let authored = crate::item_assets::test_fps_item_package();
        let policy = FpsGameplayPolicySnapshot {
            content: serde_json::to_value(authored)
                .map_err(|error| format!("test item package JSON encode failed: {error}"))?,
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
