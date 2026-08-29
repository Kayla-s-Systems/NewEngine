#[derive(Clone, Debug, Default)]
pub struct RuntimeFrameContext {
    pub frame_index: u64,
    pub dt_seconds: f32,
    pub world_playable: bool,
    pub route_trace_enabled: bool,
}
