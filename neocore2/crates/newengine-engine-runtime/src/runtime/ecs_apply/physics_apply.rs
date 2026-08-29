#[derive(Clone, Debug, Default)]
pub struct PhysicsApplyTrace {
    pub declarations: u32,
    pub applied_events: u32,
    pub diagnostics: Vec<String>,
}
