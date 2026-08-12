#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiNodeRequestAck {
    pub ok: bool,
    pub provider: Option<String>,
    pub surface_id: String,
    pub accepted_nodes: usize,
    pub warnings: Vec<String>,
}

impl Default for UiNodeRequestAck {
    fn default() -> Self {
        Self {
            ok: true,
            provider: None,
            surface_id: String::new(),
            accepted_nodes: 0,
            warnings: Vec::new(),
        }
    }
}

impl UiNodeRequestAck {
    #[inline]
    pub fn accepted(
        provider: impl Into<String>,
        surface_id: impl Into<String>,
        accepted_nodes: usize,
    ) -> Self {
        Self {
            ok: true,
            provider: Some(provider.into()),
            surface_id: surface_id.into(),
            accepted_nodes,
            warnings: Vec::new(),
        }
    }
}
