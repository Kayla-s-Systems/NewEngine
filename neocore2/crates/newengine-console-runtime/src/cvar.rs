#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum CVarValue {
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
}

impl CVarValue {
    pub fn type_id(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::I64(_) => "i64",
            Self::F64(_) => "f64",
            Self::String(_) => "string",
        }
    }

    pub fn display_value(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::I64(value) => value.to_string(),
            Self::F64(value) => value.to_string(),
            Self::String(value) => value.clone(),
        }
    }

    fn parse_like(&self, raw: &str) -> Result<Self, String> {
        match self {
            Self::Bool(_) => match raw.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "on" | "yes" => Ok(Self::Bool(true)),
                "0" | "false" | "off" | "no" => Ok(Self::Bool(false)),
                _ => Err(format!("expected bool, got '{raw}'")),
            },
            Self::I64(_) => raw
                .trim()
                .parse::<i64>()
                .map(Self::I64)
                .map_err(|_| format!("expected i64, got '{raw}'")),
            Self::F64(_) => raw
                .trim()
                .parse::<f64>()
                .map(Self::F64)
                .map_err(|_| format!("expected f64, got '{raw}'")),
            Self::String(_) => Ok(Self::String(raw.to_owned())),
        }
    }

    fn numeric(&self) -> Option<f64> {
        match self {
            Self::I64(value) => Some(*value as f64),
            Self::F64(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CVarFlags {
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub developer: bool,
    #[serde(default)]
    pub replicated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CVarDescriptor {
    pub id: String,
    pub default: CVarValue,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub flags: CVarFlags,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub persistence: String,
}

impl CVarDescriptor {
    pub fn new(id: impl Into<String>, default: CVarValue) -> Self {
        Self {
            id: id.into(),
            default,
            description: String::new(),
            min: None,
            max: None,
            flags: CVarFlags::default(),
            owner: "engine".to_owned(),
            persistence: "session".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CVarSnapshot {
    pub descriptor: CVarDescriptor,
    pub value: CVarValue,
}

#[derive(Default)]
pub struct CVarRegistry {
    entries: RwLock<BTreeMap<String, CVarSnapshot>>,
}

impl CVarRegistry {
    pub fn register(&self, mut descriptor: CVarDescriptor) -> Result<(), String> {
        descriptor.id = normalize_cvar_id(&descriptor.id)?;
        validate_descriptor(&descriptor)?;
        let value = descriptor.default.clone();
        let mut entries = self
            .entries
            .write()
            .map_err(|_| "cvar registry lock poisoned".to_owned())?;
        entries.insert(descriptor.id.clone(), CVarSnapshot { descriptor, value });
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<CVarValue, String> {
        let id = normalize_cvar_id(id)?;
        self.entries
            .read()
            .map_err(|_| "cvar registry lock poisoned".to_owned())?
            .get(&id)
            .map(|entry| entry.value.clone())
            .ok_or_else(|| format!("unknown cvar: {id}"))
    }

    pub fn set_from_str(&self, id: &str, raw: &str) -> Result<CVarValue, String> {
        let id = normalize_cvar_id(id)?;
        let mut entries = self
            .entries
            .write()
            .map_err(|_| "cvar registry lock poisoned".to_owned())?;
        let entry = entries
            .get_mut(&id)
            .ok_or_else(|| format!("unknown cvar: {id}"))?;
        if entry.descriptor.flags.read_only {
            return Err(format!("cvar is read-only: {id}"));
        }
        let value = entry.value.parse_like(raw)?;
        validate_value(&entry.descriptor, &value)?;
        entry.value = value.clone();
        Ok(value)
    }

    pub fn snapshots(&self) -> Vec<CVarSnapshot> {
        self.entries
            .read()
            .map(|entries| entries.values().cloned().collect())
            .unwrap_or_default()
    }
}

pub fn global_cvar_registry() -> Arc<CVarRegistry> {
    static REGISTRY: OnceLock<Arc<CVarRegistry>> = OnceLock::new();
    Arc::clone(REGISTRY.get_or_init(|| Arc::new(CVarRegistry::default())))
}

pub fn register_cvar(descriptor: CVarDescriptor) -> Result<(), String> {
    global_cvar_registry().register(descriptor)
}

pub trait CVarType: Sized {
    fn from_value(value: CVarValue) -> Result<Self, String>;
}

impl CVarType for bool {
    fn from_value(value: CVarValue) -> Result<Self, String> {
        match value {
            CVarValue::Bool(value) => Ok(value),
            other => Err(format!("expected bool, got {}", other.type_id())),
        }
    }
}
impl CVarType for i64 {
    fn from_value(value: CVarValue) -> Result<Self, String> {
        match value {
            CVarValue::I64(value) => Ok(value),
            other => Err(format!("expected i64, got {}", other.type_id())),
        }
    }
}
impl CVarType for f64 {
    fn from_value(value: CVarValue) -> Result<Self, String> {
        match value {
            CVarValue::F64(value) => Ok(value),
            other => Err(format!("expected f64, got {}", other.type_id())),
        }
    }
}
impl CVarType for String {
    fn from_value(value: CVarValue) -> Result<Self, String> {
        match value {
            CVarValue::String(value) => Ok(value),
            other => Err(format!("expected string, got {}", other.type_id())),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CVarHandle<T> {
    id: &'static str,
    _marker: PhantomData<fn() -> T>,
}

impl<T: CVarType> CVarHandle<T> {
    pub const fn new(id: &'static str) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    pub fn get(self) -> Result<T, String> {
        T::from_value(global_cvar_registry().get(self.id)?)
    }
}

fn normalize_cvar_id(value: &str) -> Result<String, String> {
    let id = value.trim().to_ascii_lowercase();
    if id.is_empty() || id.contains(char::is_whitespace) || id.contains('/') || id.contains('\\') {
        return Err(format!("invalid cvar id '{value}'"));
    }
    Ok(id)
}

fn validate_descriptor(descriptor: &CVarDescriptor) -> Result<(), String> {
    if let (Some(min), Some(max)) = (descriptor.min, descriptor.max) {
        if !min.is_finite() || !max.is_finite() || min > max {
            return Err(format!(
                "invalid cvar range for '{}': min={min} max={max}",
                descriptor.id
            ));
        }
    }
    validate_value(descriptor, &descriptor.default)
}

fn validate_value(descriptor: &CVarDescriptor, value: &CVarValue) -> Result<(), String> {
    if value.type_id() != descriptor.default.type_id() {
        return Err(format!("cvar '{}' type mismatch", descriptor.id));
    }
    if let Some(number) = value.numeric() {
        if !number.is_finite() {
            return Err(format!("cvar '{}' must be finite", descriptor.id));
        }
        if descriptor.min.is_some_and(|min| number < min) {
            return Err(format!("cvar '{}' is below minimum", descriptor.id));
        }
        if descriptor.max.is_some_and(|max| number > max) {
            return Err(format!("cvar '{}' is above maximum", descriptor.id));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_handle_reads_registered_value_and_range_is_enforced() {
        let registry = CVarRegistry::default();
        let mut descriptor = CVarDescriptor::new("test.speed", CVarValue::F64(1.0));
        descriptor.min = Some(0.0);
        descriptor.max = Some(4.0);
        registry.register(descriptor).expect("register");
        assert_eq!(
            registry.set_from_str("test.speed", "2.5").unwrap(),
            CVarValue::F64(2.5)
        );
        assert!(registry.set_from_str("test.speed", "8").is_err());
    }
}
