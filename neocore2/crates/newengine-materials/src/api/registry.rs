use crate::api::{MaterialDescriptor, MaterialId};

/// A snapshot item for editor UI with deterministic ordering.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSnapshotItem {
    /// Stable material identifier.
    pub id: MaterialId,
    /// Human-readable material name.
    pub name: String,
}

/// Provider that can supply a material descriptor for registration.
///
/// Plugins can implement this trait and register provider outputs into the runtime registry.
pub trait MaterialProvider: Send + Sync {
    /// Provider-visible material name.
    fn name(&self) -> &str;
    /// Material descriptor emitted by the provider.
    fn descriptor(&self) -> MaterialDescriptor;
}

/// Public registry interface.
///
/// Keep this contract minimal; the concrete implementation lives in `core`.
pub trait MaterialRegistryApi: Send + Sync {
    /// Returns a stable snapshot suitable for editor UI and inspection tools.
    fn snapshot(&self) -> Vec<MaterialSnapshotItem>;
    /// Returns a material descriptor by id.
    fn get(&self, id: MaterialId) -> Option<MaterialDescriptor>;
    /// Returns a human-readable material name by id.
    fn name(&self, id: MaterialId) -> Option<String>;
}
