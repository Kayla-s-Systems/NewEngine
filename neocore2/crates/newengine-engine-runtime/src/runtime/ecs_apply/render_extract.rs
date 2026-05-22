#[derive(Clone, Debug, Default)]
pub struct RenderExtractTrace {
    pub packets_requested: u32,
    pub draw_lists_requested: u32,
    pub diagnostics: Vec<String>,
}
