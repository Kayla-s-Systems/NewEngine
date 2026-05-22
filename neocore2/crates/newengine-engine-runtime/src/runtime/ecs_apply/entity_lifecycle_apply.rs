#[derive(Clone, Debug, Default)]
pub struct EntityLifecycleApplyTrace {
    pub spawned: u32,
    pub despawned: u32,
    pub diagnostics: Vec<String>,
}
