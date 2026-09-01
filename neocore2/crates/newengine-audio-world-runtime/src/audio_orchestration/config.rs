use std::fs;

use newengine_core::StartupConfig;

pub const AUDIO_ORCHESTRATION_RUNTIME_CONFIG_PATH: &str = "audio/orchestration.runtime.json";

const SCHEMA_VERSION: u64 = 1;
const MAX_COMMAND_CAPACITY: usize = 1_048_576;
const MAX_TRANSPORT_EVENT_CAPACITY: usize = 1_048_576;
const MAX_PROVIDER_PREARM_BLOCKS: u32 = 64;

/// Operational policy for the world-audio orchestration runtime.
///
/// These values are deliberately data-driven: production composition loads them from
/// `CONFIG/audio/orchestration.runtime.json`. `Default` exists for tests, tools and hosts that do
/// not materialize a durable CONFIG root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioOrchestrationRuntimeConfig {
    pub command_capacity: usize,
    pub command_initial_reserve: usize,
    pub transport_event_capacity: usize,
    pub provider_prearm_blocks: u32,
}

impl Default for AudioOrchestrationRuntimeConfig {
    fn default() -> Self {
        Self {
            command_capacity: 2_048,
            command_initial_reserve: 256,
            transport_event_capacity: 4_096,
            provider_prearm_blocks: 2,
        }
    }
}

impl AudioOrchestrationRuntimeConfig {
    pub fn validate(self) -> Result<Self, String> {
        if self.command_capacity == 0 || self.command_capacity > MAX_COMMAND_CAPACITY {
            return Err(format!(
                "audio orchestration command_capacity must be in 1..={MAX_COMMAND_CAPACITY}"
            ));
        }
        if self.command_initial_reserve > self.command_capacity {
            return Err(
                "audio orchestration command_initial_reserve must not exceed command_capacity"
                    .to_owned(),
            );
        }
        if self.transport_event_capacity == 0
            || self.transport_event_capacity > MAX_TRANSPORT_EVENT_CAPACITY
        {
            return Err(format!(
                "audio orchestration transport_event_capacity must be in 1..={MAX_TRANSPORT_EVENT_CAPACITY}"
            ));
        }
        if self.provider_prearm_blocks == 0
            || self.provider_prearm_blocks > MAX_PROVIDER_PREARM_BLOCKS
        {
            return Err(format!(
                "audio orchestration provider_prearm_blocks must be in 1..={MAX_PROVIDER_PREARM_BLOCKS}"
            ));
        }
        Ok(self)
    }

    pub fn load(startup: &StartupConfig) -> Result<Self, String> {
        let path = startup.config_child(AUDIO_ORCHESTRATION_RUNTIME_CONFIG_PATH);
        if !path.is_file() {
            return Self::default().validate();
        }
        let text = fs::read_to_string(&path).map_err(|error| {
            format!(
                "failed to read audio orchestration runtime config '{}': {error}",
                path.display()
            )
        })?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
            format!(
                "failed to parse audio orchestration runtime config '{}': {error}",
                path.display()
            )
        })?;
        Self::from_json(&value).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        let object = value.as_object().ok_or_else(|| {
            "audio orchestration runtime config root must be an object".to_owned()
        })?;
        const KEYS: &[&str] = &[
            "schema_version",
            "command_capacity",
            "command_initial_reserve",
            "transport_event_capacity",
            "provider_prearm_blocks",
        ];
        if let Some(key) = object.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(format!(
                "unknown audio orchestration runtime config key '{key}'"
            ));
        }
        let schema_version = object
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| "schema_version must be an unsigned integer".to_owned())?;
        if schema_version != SCHEMA_VERSION {
            return Err(format!(
                "unsupported audio orchestration runtime config schema_version={schema_version}; expected {SCHEMA_VERSION}"
            ));
        }

        let mut config = Self::default();
        if let Some(value) = object.get("command_capacity") {
            config.command_capacity = usize_from_json(value, "command_capacity")?;
        }
        if let Some(value) = object.get("command_initial_reserve") {
            config.command_initial_reserve = usize_from_json(value, "command_initial_reserve")?;
        }
        if let Some(value) = object.get("transport_event_capacity") {
            config.transport_event_capacity = usize_from_json(value, "transport_event_capacity")?;
        }
        if let Some(value) = object.get("provider_prearm_blocks") {
            config.provider_prearm_blocks = u32_from_json(value, "provider_prearm_blocks")?;
        }
        config.validate()
    }
}

fn usize_from_json(value: &serde_json::Value, name: &str) -> Result<usize, String> {
    let raw = value
        .as_u64()
        .ok_or_else(|| format!("{name} must be an unsigned integer"))?;
    usize::try_from(raw).map_err(|_| format!("{name} does not fit usize"))
}

fn u32_from_json(value: &serde_json::Value, name: &str) -> Result<u32, String> {
    let raw = value
        .as_u64()
        .ok_or_else(|| format!("{name} must be an unsigned integer"))?;
    u32::try_from(raw).map_err(|_| format!("{name} does not fit u32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_policy_overrides_operational_limits() {
        let value = serde_json::json!({
            "schema_version": 1,
            "command_capacity": 8192,
            "command_initial_reserve": 1024,
            "transport_event_capacity": 16384,
            "provider_prearm_blocks": 3
        });
        let config = AudioOrchestrationRuntimeConfig::from_json(&value).expect("config");
        assert_eq!(config.command_capacity, 8_192);
        assert_eq!(config.command_initial_reserve, 1_024);
        assert_eq!(config.transport_event_capacity, 16_384);
        assert_eq!(config.provider_prearm_blocks, 3);
    }

    #[test]
    fn unknown_policy_keys_fail_closed() {
        let value = serde_json::json!({
            "schema_version": 1,
            "command_capacity": 2048,
            "magic_latency_fix": true
        });
        assert!(AudioOrchestrationRuntimeConfig::from_json(&value).is_err());
    }
}
