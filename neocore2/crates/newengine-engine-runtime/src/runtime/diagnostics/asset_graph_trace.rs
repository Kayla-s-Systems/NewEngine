#[derive(Clone, Debug, Default)]
pub struct RuntimeAssetGraphTrace {
    pub root_ref: String,
    pub resolved_nodes: usize,
    pub missing_refs: Vec<String>,
    pub cache_key_parts: Vec<String>,
}
