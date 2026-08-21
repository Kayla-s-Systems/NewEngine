use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::validation::ensure_optional_non_blank;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectPluginRef {
    pub id: String,
    pub path: Option<PathBuf>,
    pub required: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectScriptBinding {
    /// Module ID from `scripting.modules`, or a direct script asset reference.
    pub module: String,
    /// Optional exported operation used by the consumer. The project owns this name.
    pub operation: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectScriptingManifest {
    /// Optional scripting runtime/provider hint (for example `lua`).
    pub runtime: Option<String>,
    /// Optional bootstrap module ID or direct asset reference. No specific ID is required.
    pub entrypoint: Option<String>,
    /// Arbitrary module registry. Keys and count are entirely project-defined.
    pub modules: BTreeMap<String, String>,
    /// Arbitrary consumer -> module/operation bindings. Consumer IDs are provider/service contracts.
    pub bindings: BTreeMap<String, ProjectScriptBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProjectScriptBinding {
    pub consumer: String,
    pub module_id: Option<String>,
    pub script_ref: String,
    pub operation: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ProjectScriptRegistry {
    runtime: Option<String>,
    entrypoint: Option<String>,
    modules: BTreeMap<String, String>,
    bindings: BTreeMap<String, ProjectScriptBinding>,
}

impl ProjectScriptRegistry {
    pub fn from_manifest(manifest: &ProjectScriptingManifest) -> Result<Self, String> {
        validate_scripting_manifest(manifest)?;
        Ok(Self {
            runtime: manifest.runtime.clone(),
            entrypoint: manifest.entrypoint.clone(),
            modules: manifest.modules.clone(),
            bindings: manifest.bindings.clone(),
        })
    }

    #[inline]
    pub fn runtime(&self) -> Option<&str> {
        self.runtime.as_deref()
    }

    #[inline]
    pub fn module_ids(&self) -> impl Iterator<Item = &str> {
        self.modules.keys().map(String::as_str)
    }

    pub fn module_ref(&self, id: &str) -> Option<String> {
        self.modules
            .get(id.trim())
            .map(|value| normalize_script_ref(value))
    }

    pub fn resolve_ref_or_module(&self, value: &str) -> Option<(Option<String>, String)> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        if let Some(reference) = self.modules.get(value) {
            return Some((Some(value.to_owned()), normalize_script_ref(reference)));
        }
        Some((None, normalize_script_ref(value)))
    }

    pub fn entrypoint(&self) -> Option<String> {
        self.entrypoint
            .as_deref()
            .and_then(|value| self.resolve_ref_or_module(value))
            .map(|(_, reference)| reference)
    }

    pub fn binding(&self, consumer: &str) -> Option<ResolvedProjectScriptBinding> {
        let consumer = consumer.trim();
        let binding = self.bindings.get(consumer)?;
        let (module_id, script_ref) = self.resolve_ref_or_module(&binding.module)?;
        Some(ResolvedProjectScriptBinding {
            consumer: consumer.to_owned(),
            module_id,
            script_ref,
            operation: binding
                .operation
                .clone()
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

fn normalize_script_ref(value: &str) -> String {
    let value = value.trim().replace('\\', "/");
    if let Some((prefix, tail)) = value.split_once(":/") {
        format!(
            "{}/{}",
            prefix.trim_matches('/'),
            tail.trim_start_matches('/')
        )
    } else {
        value
    }
}

pub(crate) fn validate_scripting_manifest(
    manifest: &ProjectScriptingManifest,
) -> Result<(), String> {
    ensure_optional_non_blank(manifest.runtime.as_deref(), || {
        "scripting.runtime must be non-empty when specified".to_owned()
    })?;
    for (id, reference) in &manifest.modules {
        if id.trim().is_empty() {
            return Err("scripting.modules contains an empty module id".to_owned());
        }
        if reference.trim().is_empty() {
            return Err(format!(
                "scripting module '{id}' has an empty asset reference"
            ));
        }
    }
    ensure_optional_non_blank(manifest.entrypoint.as_deref(), || {
        "scripting.entrypoint must be non-empty when specified".to_owned()
    })?;
    for (consumer, binding) in &manifest.bindings {
        if consumer.trim().is_empty() {
            return Err("scripting.bindings contains an empty consumer id".to_owned());
        }
        if binding.module.trim().is_empty() {
            return Err(format!(
                "scripting binding '{consumer}' has an empty module"
            ));
        }
        ensure_optional_non_blank(binding.operation.as_deref(), || {
            format!("scripting binding '{consumer}' has an empty operation")
        })?;
    }
    Ok(())
}
