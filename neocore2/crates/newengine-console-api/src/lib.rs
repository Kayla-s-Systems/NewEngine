#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_COMMAND_GATEWAY_ID: &str = "engine.command";
pub const COMMAND_PROVIDER_SERVICE_ID: &str = "newengine.console.command";
pub const COMMAND_PROVIDER_ROUTE: &str = "engine.command.console";
pub const COMMAND_BACKEND_CAPABILITY_ID: &str = "newengine.console.command.v1";
pub const COMMAND_SERVICE_KIND: &str = "command";
pub const COMMAND_DESCRIPTOR_CONTRACT_ID: &str = "newengine.command-descriptor/v1";

/// Compatibility name for consumers that historically treated the public
/// gateway as a concrete service id.
pub const COMMAND_SERVICE_ID: &str = ENGINE_COMMAND_GATEWAY_ID;

pub mod method {
    pub const EXEC: &str = "command.exec";
    pub const COMPLETE: &str = "command.complete";
    pub const SUGGEST: &str = "command.suggest";
    pub const REFRESH: &str = "command.refresh";
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandArgSpec {
    pub name: String,
    #[serde(default)]
    pub value_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandFlags {
    #[serde(default)]
    pub developer: bool,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub remote_allowed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandDescriptor {
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub usage: String,
    #[serde(default)]
    pub args: Vec<CommandArgSpec>,
    #[serde(default)]
    pub flags: CommandFlags,
    #[serde(default)]
    pub owner: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_gateway_and_provider_identity_are_distinct() {
        assert_eq!(ENGINE_COMMAND_GATEWAY_ID, COMMAND_SERVICE_ID);
        assert_ne!(ENGINE_COMMAND_GATEWAY_ID, COMMAND_PROVIDER_SERVICE_ID);
        assert!(COMMAND_PROVIDER_SERVICE_ID.starts_with("newengine."));
        assert_eq!(COMMAND_PROVIDER_ROUTE, "engine.command.console");
        assert!(COMMAND_PROVIDER_ROUTE.starts_with(ENGINE_COMMAND_GATEWAY_ID));
    }
}
