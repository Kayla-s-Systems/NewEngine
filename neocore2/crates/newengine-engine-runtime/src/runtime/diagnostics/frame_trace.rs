#[derive(Clone, Debug, Default)]
pub struct RuntimeFrameTrace {
    pub frame_index: u64,
    pub phases: Vec<String>,
    pub warnings: Vec<String>,
}
