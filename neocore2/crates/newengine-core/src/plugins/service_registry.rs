#![forbid(unsafe_op_in_unsafe_fn)]

use crate::plugins::describe::{parse_describe, ServiceDescribe};
use newengine_plugin_api::ServiceV1Dyn;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Strongly-typed service identifier.
///
/// Why:
/// - Avoids `String` being used as an "everything id"
/// - Enables future validation, namespacing, and typed APIs without touching call-sites
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ServiceId(Box<str>);

impl ServiceId {
    #[inline]
    pub fn new(raw: &str) -> Result<Self, String> {
        let s = raw.trim();
        if s.is_empty() {
            return Err("service id is empty".to_string());
        }
        Ok(Self(s.into()))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for ServiceId {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ServiceId").field(&self.as_str()).finish()
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Cached service metadata stored by the host for fast queries.
///
/// Intentionally NOT Clone: this is an internal registry record.
/// Use `snapshot()` for copyable data.
pub struct ServiceEntry {
    pub id: ServiceId,
    pub svc: Arc<ServiceV1Dyn<'static>>,
    pub describe_json: String,
    pub describe: Option<ServiceDescribe>,
}

impl ServiceEntry {
    #[inline]
    pub fn kind(&self) -> Option<&str> {
        self.describe.as_ref()?.kind.as_deref()
    }
}

/// Lightweight cloneable snapshot (no ABI objects inside).
#[derive(Debug, Clone)]
pub struct ServiceEntrySnapshot {
    pub id: String,
    pub kind: Option<String>,
    pub describe_json: String,
}

/// Engine-side service registry.
///
/// Notes:
/// - This is an *engine internal* registry; ABI remains string-based.
/// - We cache `describe()` because it is commonly used for capabilities/importer routing.
pub struct ServiceRegistry {
    by_id: HashMap<ServiceId, ServiceEntry>,
}

impl ServiceRegistry {
    #[inline]
    pub fn new() -> Self {
        Self {
            by_id: HashMap::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    #[inline]
    pub fn contains(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn register(&mut self, svc: ServiceV1Dyn<'static>) -> Result<ServiceId, String> {
        let raw_id = svc.id().to_string();
        let id = ServiceId::new(&raw_id)?;

        if self.by_id.contains_key(id.as_str()) {
            return Err(format!("service already registered: {}", raw_id));
        }

        let describe_json = svc.describe().to_string();
        let describe = parse_describe(&describe_json);

        let entry = ServiceEntry {
            id: id.clone(),
            svc: Arc::new(svc),
            describe_json,
            describe,
        };

        self.by_id.insert(id.clone(), entry);
        Ok(id)
    }

    #[inline]
    pub fn get(&self, id: &str) -> Option<Arc<ServiceV1Dyn<'static>>> {
        self.by_id.get(id).map(|e| e.svc.clone())
    }

    #[inline]
    pub fn get_entry(&self, id: &str) -> Option<&ServiceEntry> {
        self.by_id.get(id)
    }

    /// Snapshot for diagnostics / console / UI.
    pub fn snapshot(&self) -> Vec<ServiceEntrySnapshot> {
        let mut out = Vec::with_capacity(self.by_id.len());
        for e in self.by_id.values() {
            out.push(ServiceEntrySnapshot {
                id: e.id.as_str().to_string(),
                kind: e.kind().map(|s| s.to_string()),
                describe_json: e.describe_json.clone(),
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }
}
