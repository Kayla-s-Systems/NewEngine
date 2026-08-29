use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use newengine_ecs::World;
use newengine_gameplay_fps_api::{
    FpsCharacterMenuPolicyProvider, FpsCharacterMenuPolicySnapshot,
    FPS_CHARACTER_MENU_POLICY_SCHEMA, FPS_CHARACTER_MENU_POLICY_VERSION,
};
use newengine_scripting_client::AssetBackedScriptClient;

pub const SCRIPT_FPS_CHARACTER_MENU_PROVIDER_ID: &str =
    "newengine.gameplay.fps.character-menu.script-policy";

pub struct ScriptFpsCharacterMenuPolicyProvider {
    client: AssetBackedScriptClient,
    operation: String,
    snapshot: OnceLock<Arc<FpsCharacterMenuPolicySnapshot>>,
}

impl ScriptFpsCharacterMenuPolicyProvider {
    pub fn new(script_ref: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            client: AssetBackedScriptClient::new(script_ref, "fps-character-menu-policy"),
            operation: operation.into(),
            snapshot: OnceLock::new(),
        }
    }

    #[inline]
    pub fn script_ref(&self) -> &str {
        self.client.script_ref()
    }

    fn load_uncached(&self) -> Result<Arc<FpsCharacterMenuPolicySnapshot>, String> {
        if self.operation.trim().is_empty() {
            return Err("character-menu script binding has no operation".to_owned());
        }
        self.client.load_module()?;
        let snapshot: FpsCharacterMenuPolicySnapshot = self.client.invoke_json_unit(
            "fps-character-menu.bootstrap.v1",
            &self.operation,
            BTreeMap::from([
                (
                    "expected_schema".to_owned(),
                    FPS_CHARACTER_MENU_POLICY_SCHEMA.to_owned(),
                ),
                (
                    "expected_version".to_owned(),
                    FPS_CHARACTER_MENU_POLICY_VERSION.to_string(),
                ),
            ]),
        )?;
        snapshot
            .validate()
            .map_err(|error| format!("Shared FPS character-menu policy invalid: {error}"))?;
        Ok(Arc::new(snapshot))
    }
}

impl FpsCharacterMenuPolicyProvider for ScriptFpsCharacterMenuPolicyProvider {
    #[inline]
    fn id(&self) -> &'static str {
        SCRIPT_FPS_CHARACTER_MENU_PROVIDER_ID
    }

    fn load_snapshot(&self) -> Result<Arc<FpsCharacterMenuPolicySnapshot>, String> {
        if let Some(snapshot) = self.snapshot.get() {
            return Ok(Arc::clone(snapshot));
        }
        let loaded = self.load_uncached()?;
        let _ = self.snapshot.set(Arc::clone(&loaded));
        Ok(self.snapshot.get().cloned().unwrap_or(loaded))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CharacterMenuPolicyLoadFailure {
    retry_after_samples: u16,
}

pub fn ensure_character_menu_policy(
    world: &mut World,
    provider: &dyn FpsCharacterMenuPolicyProvider,
) {
    if world.resource::<FpsCharacterMenuPolicySnapshot>().is_some() {
        return;
    }
    if let Some(failure) = world.resource_mut::<CharacterMenuPolicyLoadFailure>() {
        if failure.retry_after_samples > 0 {
            failure.retry_after_samples -= 1;
            return;
        }
    }
    match provider.load_snapshot() {
        Ok(snapshot) => {
            newengine_ulog_api::ulog::info!(
                "fps character menu policy installed provider='{}' toggle_action='{}' shared_characters={}",
                provider.id(),
                snapshot.toggle_action,
                snapshot.characters.len()
            );
            world.remove_resource::<CharacterMenuPolicyLoadFailure>();
            world.insert_resource((*snapshot).clone());
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps character menu policy unavailable provider='{}': {}",
                provider.id(),
                error
            );
            world.insert_resource(CharacterMenuPolicyLoadFailure {
                retry_after_samples: 60,
            });
        }
    }
}
