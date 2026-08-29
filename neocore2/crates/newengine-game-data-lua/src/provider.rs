use newengine_game_data::{GameData, GameDataProvider, GameDataSnapshot};

use crate::client::load_game_data_from_script;

pub const SCRIPT_GAME_DATA_PROVIDER_ID: &str = "newengine.game_data.script";
pub const LUA_GAME_DATA_PROVIDER_ID: &str = "newengine.game_data.lua";

#[derive(Clone, Debug)]
pub struct LuaGameDataProvider {
    script_ref: String,
    operation: String,
}

impl LuaGameDataProvider {
    #[inline]
    pub fn new(script_ref: impl Into<String>) -> Self {
        Self {
            script_ref: script_ref.into(),
            operation: String::new(),
        }
    }

    #[inline]
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = operation.into();
        self
    }

    #[inline]
    pub fn script_ref(&self) -> &str {
        &self.script_ref
    }

    #[inline]
    pub fn operation(&self) -> &str {
        &self.operation
    }

    fn source_id(&self) -> String {
        format!(
            "{}:{}#{}",
            SCRIPT_GAME_DATA_PROVIDER_ID, self.script_ref, self.operation
        )
    }
}

impl GameDataProvider for LuaGameDataProvider {
    #[inline]
    fn id(&self) -> &'static str {
        SCRIPT_GAME_DATA_PROVIDER_ID
    }

    fn load(&self) -> Result<GameData, String> {
        if self.operation.trim().is_empty() {
            return Err(format!(
                "Script game-data provider '{}' has no configured operation; bind one in the project scripting registry",
                self.script_ref
            ));
        }
        load_game_data_from_script(&self.script_ref, &self.operation)
    }

    fn load_snapshot(&self) -> Result<GameDataSnapshot, String> {
        let data = self.load()?;
        Ok(GameDataSnapshot::new(self.source_id(), data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_provider_targets_selectorless_game_data_module() {
        let provider = LuaGameDataProvider::new("scripts/custom_data.ysc")
            .with_operation("custom_data_export");
        assert_eq!(provider.id(), SCRIPT_GAME_DATA_PROVIDER_ID);
        assert_eq!(provider.script_ref(), "scripts/custom_data.ysc");
        assert!(!provider.script_ref().contains('@'));
        assert_eq!(provider.operation(), "custom_data_export");
    }
}
