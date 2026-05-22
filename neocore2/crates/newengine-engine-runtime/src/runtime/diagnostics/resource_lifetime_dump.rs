#[derive(Clone, Debug, Default)]
pub struct RuntimeResourceLifetimeDump {
    pub created_resources: u64,
    pub destroyed_resources: u64,
    pub persistent_resources: u64,
    pub explanation: Vec<String>,
}
