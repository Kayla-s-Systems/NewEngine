#[derive(Clone, Debug)]
pub enum RuntimeFrameEvent {
    PhaseStarted {
        phase: &'static str,
        frame_index: u64,
    },
    PhaseCompleted {
        phase: &'static str,
        frame_index: u64,
        elapsed_ms: f32,
    },
    Diagnostic {
        domain: &'static str,
        message: String,
    },
}
