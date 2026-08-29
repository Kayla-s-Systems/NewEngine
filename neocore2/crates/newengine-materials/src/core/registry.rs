use crate::api::{
    bump_id, material_id_from_name, material_instance_id, MaterialDescriptor, MaterialId,
    MaterialInstanceDesc, MaterialOverrides, MaterialProvider, MaterialRegistryApi,
    MaterialResolved, MaterialSnapshotItem, MaterialTextureBindings,
};
use crate::errors::{MaterialError, MaterialResult};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug)]
enum EntryKind {
    Asset {
        desc: MaterialDescriptor,
        textures: MaterialTextureBindings,
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

#[derive(Clone, Default)]
pub struct MaterialRegistry {
    inner: Arc<RwLock<Vec<Entry>>>,
    revision: Arc<AtomicU64>,
}

impl MaterialRegistry {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    #[inline]
    fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    #[inline]
    pub fn with_builtins() -> Self {
        let reg = Self::new();
        crate::builtin::register_all(&reg);
        reg
    }

    #[inline]
    pub fn ids(&self) -> Vec<MaterialId> {
        self.inner.read().iter().map(|e| e.id).collect()
    }

    pub fn register_named(&self, name: &str, desc: MaterialDescriptor) -> MaterialId {
        self.register_named_with_textures(name, desc, MaterialTextureBindings::default())
    }

    pub fn register_named_with_textures(
        &self,
        name: &str,
        mut desc: MaterialDescriptor,
        textures: MaterialTextureBindings,
    ) -> MaterialId {
        {
            let v = self.inner.read();
            if let Some(e) = v.iter().find(|e| e.name == name) {
                return e.id;
            }
        }

        desc.sanitize_in_place();
        let textures = textures.sanitized();
        let mut id = material_id_from_name(name);

        let mut v = self.inner.write();
        while v.iter().any(|e| e.id == id) {
            id = bump_id(id);
        }

        v.push(Entry {
            id,
            name: name.to_string(),
            kind: EntryKind::Asset { desc, textures },
        });
        drop(v);
        self.bump_revision();

        id
    }

    pub fn upsert_named(&self, name: &str, desc: MaterialDescriptor) -> MaterialId {
        self.upsert_named_with_textures(name, desc, MaterialTextureBindings::default())
    }

    pub fn upsert_named_with_textures(
        &self,
        name: &str,
        mut desc: MaterialDescriptor,
        textures: MaterialTextureBindings,
    ) -> MaterialId {
        desc.sanitize_in_place();
        let textures = textures.sanitized();
        {
            let mut v = self.inner.write();
            if let Some(e) = v.iter_mut().find(|e| e.name == name) {
                match &mut e.kind {
                    EntryKind::Asset {
                        desc: cur,
                        textures: cur_tex,
                    } => {
                        *cur = desc;
                        *cur_tex = textures;
                    }
                    EntryKind::Instance { .. } => {}
                }
                let id = e.id;
                drop(v);
                self.bump_revision();
                return id;
            }
        }

        self.register_named_with_textures(name, desc, textures)
    }

    #[inline]
    pub fn register_provider(&self, provider: &dyn MaterialProvider) -> MaterialId {
        self.register_named(provider.name(), provider.descriptor())
    }

    pub fn set_desc(&self, id: MaterialId, mut desc: MaterialDescriptor) -> MaterialResult<()> {
        if !id.is_valid() || id.is_instance() {
            return Err(MaterialError::InvalidId);
        }

        desc.sanitize_in_place();
        let mut v = self.inner.write();
        let Some(e) = v.iter_mut().find(|e| e.id == id) else {
            return Err(MaterialError::NotFound);
        };

        match &mut e.kind {
            EntryKind::Asset { desc: cur, .. } => {
                *cur = desc;
                drop(v);
                self.bump_revision();
                Ok(())
            }
            EntryKind::Instance { .. } => Err(MaterialError::InvalidId),
        }
    }

    pub fn set_textures(
        &self,
        id: MaterialId,
        textures: MaterialTextureBindings,
    ) -> MaterialResult<()> {
        if !id.is_valid() || id.is_instance() {
            return Err(MaterialError::InvalidId);
        }

        let textures = textures.sanitized();
        let mut v = self.inner.write();
        let Some(e) = v.iter_mut().find(|e| e.id == id) else {
            return Err(MaterialError::NotFound);
        };

        match &mut e.kind {
            EntryKind::Asset { textures: cur, .. } => {
                *cur = textures;
                drop(v);
                self.bump_revision();
                Ok(())
            }
            EntryKind::Instance { .. } => Err(MaterialError::InvalidId),
        }
    }

    pub fn register_instance_named(
        &self,
        base: MaterialId,
        name: &str,
        overrides: MaterialOverrides,
    ) -> MaterialId {
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
        drop(v);
        self.bump_revision();

        id
    }

    pub fn upsert_instance_named(
        &self,
        base: MaterialId,
        name: &str,
        overrides: MaterialOverrides,
    ) -> MaterialId {
        let mut v = self.inner.write();
        if let Some(e) = v.iter_mut().find(|e| e.name == name) {
            match &mut e.kind {
                EntryKind::Instance {
                    base: b,
                    overrides: o,
                } => {
                    *b = base;
                    *o = overrides;
                }
                EntryKind::Asset { .. } => {}
            }
            let id = e.id;
            drop(v);
            self.bump_revision();
            return id;
        }

        drop(v);
        self.register_instance_named(base, name, overrides)
    }

    #[inline]
    pub fn register_instance(&self, name: &str, inst: MaterialInstanceDesc) -> MaterialId {
        self.register_instance_named(inst.base, name, inst.overrides)
    }

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
                drop(v);
                self.bump_revision();
                Ok(())
            }
            _ => Err(MaterialError::InvalidId),
        }
    }

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
        drop(v);
        self.bump_revision();

        Ok(())
    }
}

impl MaterialRegistryApi for MaterialRegistry {
    #[inline]
    fn revision(&self) -> u64 {
        MaterialRegistry::revision(self)
    }

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
        self.resolve(id).map(|v| v.desc)
    }

    #[inline]
    fn textures(&self, id: MaterialId) -> Option<MaterialTextureBindings> {
        self.resolve(id).map(|v| v.textures)
    }

    #[inline]
    fn resolve(&self, id: MaterialId) -> Option<MaterialResolved> {
        let v = self.inner.read();
        let e = v.iter().find(|e| e.id == id)?;
        match &e.kind {
            EntryKind::Asset { desc, textures } => Some(MaterialResolved {
                id: e.id,
                desc: *desc,
                textures: textures.clone(),
            }),
            EntryKind::Instance { base, overrides } => {
                let base_e = v.iter().find(|x| x.id == *base)?;
                let (base_desc, base_textures) = match &base_e.kind {
                    EntryKind::Asset { desc, textures } => (*desc, textures.clone()),
                    _ => return None,
                };
                Some(MaterialResolved {
                    id: e.id,
                    desc: overrides.apply_to(base_desc),
                    textures: base_textures,
                })
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
