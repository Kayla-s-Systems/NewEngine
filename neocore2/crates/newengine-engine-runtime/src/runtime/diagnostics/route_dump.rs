#[derive(Clone, Debug, Default)]
pub struct RuntimeRouteDump {
    pub active_routes: Vec<String>,
    pub shadowed_routes: Vec<String>,
}
