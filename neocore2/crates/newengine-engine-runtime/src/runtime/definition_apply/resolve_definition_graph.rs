#[derive(Clone, Debug, Default)]
pub struct ResolvedDefinitionGraphTrace {
    pub definition_ref: String,
    pub graph_root_ref: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub missing_refs: Vec<String>,
    pub stable_cache_key: String,
}
