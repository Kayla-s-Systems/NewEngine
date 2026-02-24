use crate::api::{
    bump_id, material_id_from_name, material_instance_id, MaterialDescriptor, MaterialId,
    MaterialInstanceDesc, MaterialOverrides, MaterialProvider, MaterialRegistryApi,
    MaterialSnapshotItem,
};
use crate::errors::{MaterialError, MaterialResult};
use parking_lot::RwLock;
use std::sync::Arc;

#[derive(Clone, Debug)]
enum EntryKind {
    Asset {
        desc: MaterialDescriptor,
    },
    Instance {
        base: MaterialId,
        overrides: MaterialOverrides,
    },
}

#[derive(Clone, Debug)]
struct Entry {
    id: MaterialId,
    name: String,
    kind: EntryKind,
}

/// Deterministic material registry.
///
/// - Stable ids derived from names (FNV-1a 64) with deterministic collision resolution.
/// - Stable iteration order: insertion order.
/// - Thread-safe via `Arc<RwLock<_>>`.
#[derive(Clone, Default)]
pub struct MaterialRegistry {
    inner: Arc<RwLock<Vec<Entry>>>,
}

impl MaterialRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry pre-populated with built-in materials.
    #[inline]
    pub fn with_builtins() -> Self {
        let reg = Self::new();
        crate::builtin::register_all(&reg);
        reg
    }

    /// Returns ids in stable insertion order.
    #[inline]
    pub fn ids(&self) -> Vec<MaterialId> {
        self.inner.read().iter().map(|e| e.id).collect()
    }

    /// Register a material descriptor by name.
    ///
    /// If a material with the same name already exists, returns its existing id.
    pub fn register_named(&self, name: &str, desc: MaterialDescriptor) -> MaterialId {
        // Fast path: already registered by name.
        {
            let v = self.inner.read();
            if let Some(e) = v.iter().find(|e| e.name == name) {
                return e.id;
            }
        }

        let mut id = material_id_from_name(name);

        let mut v = self.inner.write();
        while v.iter().any(|e| e.id == id) {
            id = bump_id(id);
        }

        v.push(Entry {
            id,
            name: name.to_string(),
            kind: EntryKind::Asset { desc },
        });

        id
    }

    /// Register a provider output.
    #[inline]
    pub fn register_provider(&self, provider: &dyn MaterialProvider) -> MaterialId {
        self.register_named(provider.name(), provider.descriptor())
    }

    /// Update descriptor for a specific id.
    pub fn set_desc(&self, id: MaterialId, desc: MaterialDescriptor) -> MaterialResult<()> {
        if !id.is_valid() {
            return Err(MaterialError::InvalidId);
        }

        if id.is_instance() {
            return Err(MaterialError::InvalidId);
        }

        let mut v = self.inner.write();
        let Some(e) = v.iter_mut().find(|e| e.id == id) else {
            return Err(MaterialError::NotFound);
        };

        match &mut e.kind {
            EntryKind::Asset { desc: cur } => {
                *cur = desc;
            }
            EntryKind::Instance { .. } => {
                return Err(MaterialError::InvalidId);
            }
        }
        Ok(())
    }

    /// Register a deterministic instance for an existing base material.
    ///
    /// If an instance with the same name already exists, returns its existing id.
    pub fn register_instance_named(
        &self,
        base: MaterialId,
        name: &str,
        overrides: MaterialOverrides,
    ) -> MaterialId {
        // Fast path by name.
        {
            let v = self.inner.read();
            if let Some(e) = v.iter().find(|e| e.name == name) {
                return e.id;
            }
        }

        let mut id = material_instance_id(base, name);

        let mut v = self.inner.write();
        while v.iter().any(|e| e.id == id) {
            id = bump_id(id);
        }

        v.push(Entry {
            id,
            name: name.to_string(),
            kind: EntryKind::Instance { base, overrides },
        });

        id
    }

    /// Register an instance by descriptor.
    #[inline]
    pub fn register_instance(&self, name: &str, inst: MaterialInstanceDesc) -> MaterialId {
        self.register_instance_named(inst.base, name, inst.overrides)
    }

    /// Update overrides for a specific instance id.
    pub fn set_instance_overrides(
        &self,
        id: MaterialId,
        overrides: MaterialOverrides,
    ) -> MaterialResult<()> {
        if !id.is_valid() || !id.is_instance() {
            return Err(MaterialError::InvalidId);
        }

        let mut v = self.inner.write();
        let Some(e) = v.iter_mut().find(|e| e.id == id) else {
            return Err(MaterialError::NotFound);
        };

        match &mut e.kind {
            EntryKind::Instance { overrides: cur, .. } => {
                *cur = overrides;
                Ok(())
            }
            _ => Err(MaterialError::InvalidId),
        }
    }

    /// Deterministic remove.
    pub fn remove(&self, id: MaterialId) -> MaterialResult<()> {
        if !id.is_valid() {
            return Err(MaterialError::InvalidId);
        }

        let mut v = self.inner.write();
        let before = v.len();
        v.retain(|e| e.id != id);

        if v.len() == before {
            return Err(MaterialError::NotFound);
        }

        Ok(())
    }
}

impl MaterialRegistryApi for MaterialRegistry {
    #[inline]
    fn snapshot(&self) -> Vec<MaterialSnapshotItem> {
        self.inner
            .read()
            .iter()
            .filter(|e| e.id.is_asset())
            .map(|e| MaterialSnapshotItem {
                id: e.id,
                name: e.name.clone(),
            })
            .collect()
    }

    #[inline]
    fn get(&self, id: MaterialId) -> Option<MaterialDescriptor> {
        let v = self.inner.read();
        let e = v.iter().find(|e| e.id == id)?;
        match &e.kind {
            EntryKind::Asset { desc } => Some(*desc),
            EntryKind::Instance { base, overrides } => {
                let base_desc = v
                    .iter()
                    .find(|x| x.id == *base)
                    .and_then(|x| match &x.kind {
                        EntryKind::Asset { desc } => Some(*desc),
                        _ => None,
                    })?;
                Some(overrides.apply_to(base_desc))
            }
        }
    }

    #[inline]
    fn name(&self, id: MaterialId) -> Option<String> {
        self.inner
            .read()
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.name.clone())
    }
}
