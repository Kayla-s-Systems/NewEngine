use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::ScriptingDiagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptModuleState {
    Declared,
    Loaded,
    Disabled,
    Failed,
}

impl Default for ScriptModuleState {
    #[inline]
    fn default() -> Self {
        Self::Declared
    }
}

pub type ScriptingModuleState = ScriptModuleState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptModuleRef {
    /// Canonical runtime script-module asset, usually `scripts/foo.ysc`.
    pub reference: String,
    /// Optional normalized module id used by tooling/runtime caches. It does not
    /// imply any language/runtime identity.
    pub module_id: String,
}

impl Default for ScriptModuleRef {
    #[inline]
    fn default() -> Self {
        Self {
            reference: String::new(),
            module_id: String::new(),
        }
    }
}

impl ScriptModuleRef {
    #[inline]
    pub fn new(reference: impl Into<String>) -> Self {
        let reference = reference.into();
        Self {
            module_id: default_module_id_from_ref(&reference),
            reference,
        }
    }

    #[inline]
    pub fn is_selector_free_module_ref(&self) -> bool {
        let reference = self.reference.trim();
        !reference.is_empty() && !reference.contains('@')
    }
}

pub type ScriptingModuleRef = ScriptModuleRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptPermission {
    /// Engine-facing permission id. It describes what the response asks the
    /// engine to do, not which provider/private language API was used.
    pub id: String,
    pub scope: String,
}

impl Default for ScriptPermission {
    #[inline]
    fn default() -> Self {
        Self {
            id: String::new(),
            scope: String::new(),
        }
    }
}

impl ScriptPermission {
    #[inline]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            scope: String::new(),
        }
    }

    #[inline]
    pub fn scoped(id: impl Into<String>, scope: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            scope: scope.into(),
        }
    }
}

pub type ScriptingPermission = ScriptPermission;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleLoadBytesRequest {
    pub module_ref: ScriptingModuleRef,
    /// Raw `.ysc` module body bytes or provider-specific module bytes. Core
    /// stores/forwards these bytes and does not interpret them.
    pub module_bytes: Vec<u8>,
    pub permissions: Vec<ScriptingPermission>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for ScriptingModuleLoadBytesRequest {
    #[inline]
    fn default() -> Self {
        Self {
            module_ref: ScriptingModuleRef::default(),
            module_bytes: Vec::new(),
            permissions: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleUnloadRequest {
    pub module_ref: ScriptingModuleRef,
}

impl Default for ScriptingModuleUnloadRequest {
    #[inline]
    fn default() -> Self {
        Self {
            module_ref: ScriptingModuleRef::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleRecord {
    pub schema: String,
    pub module_ref: ScriptingModuleRef,
    pub state: ScriptingModuleState,
    pub permissions: Vec<ScriptingPermission>,
    pub module_bytes_len: u64,
    pub metadata: BTreeMap<String, String>,
    pub diagnostics: Vec<ScriptingDiagnostic>,
}

impl Default for ScriptingModuleRecord {
    #[inline]
    fn default() -> Self {
        Self {
            schema: "newengine.scripting.module_record.v1".to_owned(),
            module_ref: ScriptingModuleRef::default(),
            state: ScriptingModuleState::Declared,
            permissions: Vec::new(),
            module_bytes_len: 0,
            metadata: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleLoadBytesResponse {
    pub ok: bool,
    pub module: ScriptingModuleRecord,
    pub diagnostics: Vec<ScriptingDiagnostic>,
}

impl Default for ScriptingModuleLoadBytesResponse {
    #[inline]
    fn default() -> Self {
        Self {
            ok: false,
            module: ScriptingModuleRecord::default(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptingModuleRefValidationResponse {
    pub ok: bool,
    pub module_ref: ScriptingModuleRef,
    pub diagnostics: Vec<ScriptingDiagnostic>,
}

impl Default for ScriptingModuleRefValidationResponse {
    #[inline]
    fn default() -> Self {
        Self {
            ok: false,
            module_ref: ScriptingModuleRef::default(),
            diagnostics: Vec::new(),
        }
    }
}

pub type ScriptModuleRefValidationResponse = ScriptingModuleRefValidationResponse;

#[inline]
pub fn default_module_id_from_ref(reference: &str) -> String {
    let mut id = reference.trim().replace('\\', "/");
    if id.is_empty() {
        return String::new();
    }
    id = id.trim_start_matches('/').to_ascii_lowercase();
    id.chars()
        .map(|ch| {
            if matches!(ch, '/' | '@' | '.') {
                '_'
            } else {
                ch
            }
        })
        .collect()
}
