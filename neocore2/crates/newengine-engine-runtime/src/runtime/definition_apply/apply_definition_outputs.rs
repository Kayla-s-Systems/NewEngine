#[derive(Clone, Debug, Default)]
pub struct DefinitionApplyResult {
    pub entity_spawn_requested: bool,
    pub render_packet_requested: bool,
    pub physics_declaration_requested: bool,
    pub diagnostics: Vec<String>,
}
