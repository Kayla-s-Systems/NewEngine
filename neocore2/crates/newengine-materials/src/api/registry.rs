use crate::api::{MaterialDescriptor, MaterialId};

/// A snapshot item for editor UI (stable order).
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSnapshotItem {
    pub id: MaterialId,
    pub name: String,
}

/// Provider that can supply a material descriptor for registration.
///
/// Plugins can implement this and register provider outputs into the registry.
pub trait MaterialProvider: Send + Sync {
    fn name(&self) -> &str;
    fn descriptor(&self) -> MaterialDescriptor;
}

/// Public registry interface.
///
/// Keep this minimal; the concrete implementation lives in `core`.
pub trait MaterialRegistryApi: Send + Sync {
    fn snapshot(&self) -> Vec<MaterialSnapshotItem>;
    fn get(&self, id: MaterialId) -> Option<MaterialDescriptor>;
    fn name(&self, id: MaterialId) -> Option<String>;
}
