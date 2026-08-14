use serde::{Deserialize, Serialize};

pub const REPLICATION_DESCRIPTOR_CONTRACT: &str = "newengine.replication-descriptor.v1";
pub const ENGINE_REPLICATION_SERVICE_ID: &str = "engine.replication";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationAuthority {
    #[default]
    Server,
    Owner,
    Shared,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationReliability {
    #[default]
    Unreliable,
    UnreliableSequenced,
    Reliable,
    ReliableOrdered,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationCondition {
    #[default]
    Relevant,
    Everyone,
    OwnerOnly,
    SkipOwner,
    InitialOnly,
    Never,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationWireType {
    Bool,
    I32,
    U32,
    I64,
    U64,
    #[default]
    F32,
    F64,
    Vec2F32,
    Vec3F32,
    Vec4F32,
    QuatF32,
    String,
    Bytes,
    Json,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplicationQuantization {
    pub enabled: bool,
    pub min: f64,
    pub max: f64,
    pub bits: u8,
}

impl Default for ReplicationQuantization {
    fn default() -> Self {
        Self {
            enabled: false,
            min: 0.0,
            max: 1.0,
            bits: 0,
        }
    }
}

impl ReplicationQuantization {
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if !self.min.is_finite() || !self.max.is_finite() || self.max <= self.min {
            return Err("replication quantization requires finite max > min".to_owned());
        }
        if !(1..=32).contains(&self.bits) {
            return Err("replication quantization bits must be 1..=32".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplicatedFieldDescriptor {
    /// Stable wire field id. Never reuse an old id for a different semantic field.
    pub field_id: u16,
    pub name: String,
    pub wire_type: ReplicationWireType,
    pub condition: ReplicationCondition,
    pub quantization: ReplicationQuantization,
    pub delta_compressed: bool,
    pub interpolate: bool,
}

impl Default for ReplicatedFieldDescriptor {
    fn default() -> Self {
        Self {
            field_id: 0,
            name: String::new(),
            wire_type: ReplicationWireType::F32,
            condition: ReplicationCondition::Relevant,
            quantization: ReplicationQuantization::default(),
            delta_compressed: true,
            interpolate: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplicatedComponentDescriptor {
    pub component_id: String,
    pub version: u32,
    pub owner: String,
    pub authority: ReplicationAuthority,
    pub channel: u8,
    pub update_hz: f32,
    pub reliability: ReplicationReliability,
    pub priority: u8,
    pub dormancy_allowed: bool,
    pub fields: Vec<ReplicatedFieldDescriptor>,
}

impl Default for ReplicatedComponentDescriptor {
    fn default() -> Self {
        Self {
            component_id: String::new(),
            version: 1,
            owner: String::new(),
            authority: ReplicationAuthority::Server,
            channel: 0,
            update_hz: 20.0,
            reliability: ReplicationReliability::UnreliableSequenced,
            priority: 128,
            dormancy_allowed: true,
            fields: Vec::new(),
        }
    }
}

impl ReplicatedComponentDescriptor {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.component_id.trim().is_empty() || self.component_id.len() > 128 {
            errors.push("component_id must contain 1..=128 bytes".to_owned());
        }
        if self.version == 0 {
            errors.push("component version must be >= 1".to_owned());
        }
        if !self.update_hz.is_finite() || !(0.1..=240.0).contains(&self.update_hz) {
            errors.push(format!(
                "component '{}' update_hz must be 0.1..=240",
                self.component_id
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        for field in &self.fields {
            if field.field_id == 0 {
                errors.push(format!(
                    "component '{}' field '{}' uses reserved field_id=0",
                    self.component_id, field.name
                ));
            }
            if !ids.insert(field.field_id) {
                errors.push(format!(
                    "component '{}' duplicate field_id={}",
                    self.component_id, field.field_id
                ));
            }
            let name = field.name.trim();
            if name.is_empty() || !names.insert(name.to_owned()) {
                errors.push(format!(
                    "component '{}' invalid/duplicate field name '{}'",
                    self.component_id, field.name
                ));
            }
            if let Err(error) = field.quantization.validate() {
                errors.push(format!(
                    "component '{}' field '{}': {error}",
                    self.component_id, field.name
                ));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplicatedEntityProfile {
    pub id: String,
    pub version: u32,
    pub owner: String,
    pub components: Vec<String>,
    pub relevancy_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplicatedMessageDescriptor {
    pub message_id: String,
    pub version: u32,
    pub channel: u8,
    pub reliability: ReplicationReliability,
    pub max_rate_hz: u16,
    pub condition: ReplicationCondition,
}

impl Default for ReplicatedMessageDescriptor {
    fn default() -> Self {
        Self {
            message_id: String::new(),
            version: 1,
            channel: 1,
            reliability: ReplicationReliability::ReliableOrdered,
            max_rate_hz: 60,
            condition: ReplicationCondition::Relevant,
        }
    }
}

pub const REPLICATION_DEFINITION_BUNDLE_SCHEMA: &str = "newengine.replication.definitions.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplicationDefinitionBundleV1 {
    pub schema: String,
    pub owner: String,
    pub components: Vec<ReplicatedComponentDescriptor>,
    pub entity_profiles: Vec<ReplicatedEntityProfile>,
    pub messages: Vec<ReplicatedMessageDescriptor>,
}

impl Default for ReplicationDefinitionBundleV1 {
    fn default() -> Self {
        Self {
            schema: REPLICATION_DEFINITION_BUNDLE_SCHEMA.to_owned(),
            owner: String::new(),
            components: Vec::new(),
            entity_profiles: Vec::new(),
            messages: Vec::new(),
        }
    }
}

impl ReplicationDefinitionBundleV1 {
    pub fn validate_header(&self) -> Result<(), String> {
        if self.schema.trim() != REPLICATION_DEFINITION_BUNDLE_SCHEMA {
            return Err(format!(
                "replication definition schema '{}' is unsupported; expected '{}'",
                self.schema, REPLICATION_DEFINITION_BUNDLE_SCHEMA
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReplicationRegistrySnapshot {
    pub contract: String,
    pub generation: u64,
    pub components: Vec<ReplicatedComponentDescriptor>,
    pub entity_profiles: Vec<ReplicatedEntityProfile>,
    pub messages: Vec<ReplicatedMessageDescriptor>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_wire_ids_must_be_unique_and_nonzero() {
        let descriptor = ReplicatedComponentDescriptor {
            component_id: "game.transform".into(),
            fields: vec![
                ReplicatedFieldDescriptor {
                    field_id: 1,
                    name: "position".into(),
                    wire_type: ReplicationWireType::Vec3F32,
                    ..Default::default()
                },
                ReplicatedFieldDescriptor {
                    field_id: 1,
                    name: "rotation".into(),
                    wire_type: ReplicationWireType::QuatF32,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(descriptor.validate().is_err());
    }

    #[test]
    fn quantized_field_requires_finite_range() {
        let q = ReplicationQuantization {
            enabled: true,
            min: -1000.0,
            max: 1000.0,
            bits: 16,
        };
        assert!(q.validate().is_ok());
    }
}
