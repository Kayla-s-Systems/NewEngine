use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDiagnostics {
    pub provider: String,
    pub output_ready: bool,
    /// Logical voices known to the provider (physical + virtual).
    pub active_voices: usize,
    pub spatial_voices: usize,
    #[serde(default)]
    pub physical_voices: usize,
    #[serde(default)]
    pub virtual_voices: usize,
    #[serde(default)]
    pub max_physical_voices: usize,
    #[serde(default)]
    pub voice_budget_reservations: std::collections::BTreeMap<String, usize>,
    #[serde(default)]
    pub attenuated_voices: usize,
    #[serde(default)]
    pub obstructed_voices: usize,
    #[serde(default)]
    pub occluded_voices: usize,
    #[serde(default)]
    pub spectrally_filtered_voices: usize,
    #[serde(default)]
    pub air_filtered_voices: usize,
    #[serde(default)]
    pub doppler_shifted_voices: usize,
    #[serde(default)]
    pub portal_attenuated_voices: usize,
    #[serde(default)]
    pub reverberant_voices: usize,
    /// Number of native shared late-field processors currently allocated by room identity.
    #[serde(default)]
    pub active_room_buses: usize,
    /// Hard bound for simultaneously resident native room late-field processors.
    #[serde(default)]
    pub max_room_buses: usize,
    /// Native callback render graph telemetry. These are provider-output PCM frames.
    #[serde(default)]
    pub render_sample: u64,
    #[serde(default)]
    pub render_block_frames: u32,
    #[serde(default)]
    pub rendered_blocks: u64,
    #[serde(default)]
    pub rendered_frames: u64,
    #[serde(default)]
    pub render_split_segments: u64,
    #[serde(default)]
    pub render_applied_commands: u64,
    #[serde(default)]
    pub render_dropped_commands: u64,
    #[serde(default)]
    pub render_active_nodes: usize,
    /// Logical long-form stream voices (physical + virtual).
    #[serde(default)]
    pub active_streams: usize,
    #[serde(default)]
    pub physical_streams: usize,
    #[serde(default)]
    pub virtual_streams: usize,
    /// Logical -> physical stream materializations, including first realization.
    #[serde(default)]
    pub stream_promotions: u64,
    /// Physical -> logical-only transitions caused by arbitration/rematerialization.
    #[serde(default)]
    pub stream_demotions: u64,
    #[serde(default)]
    pub stream_buffered_frames: usize,
    #[serde(default)]
    pub stream_buffer_capacity_frames: usize,
    #[serde(default)]
    pub stream_underruns: u64,
    #[serde(default)]
    pub stream_range_requests: u64,
    #[serde(default)]
    pub stream_compressed_bytes_fetched: u64,
    #[serde(default)]
    pub stream_seek_operations: u64,
    /// Decoded/validated YSNCD SoundGraphs currently resident in the cue cache.
    #[serde(default)]
    pub cached_sound_graphs: usize,
    /// Persistent Sequence cursor states, scoped by cue/node/object.
    #[serde(default)]
    pub sound_graph_sequence_states: usize,
    pub cached_clips: usize,
    pub cached_bytes: usize,
    pub listener: AudioListenerState,
    #[serde(default)]
    pub listener_velocity: [f32; 3],
    #[serde(default)]
    pub route_gains: std::collections::BTreeMap<String, f32>,
}
