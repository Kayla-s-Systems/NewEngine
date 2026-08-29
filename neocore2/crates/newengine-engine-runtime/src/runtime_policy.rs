#![forbid(unsafe_op_in_unsafe_fn)]

//! Immutable startup policy snapshots for frame-critical runtime systems.
//!
//! Process environment is a bootstrap/config compatibility boundary, not a frame
//! API. Values in this module are normalized once and then consumed as typed
//! policy by render, simulation, diagnostics and streaming paths.

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderJobEventMode {
    Off,
    Sampled,
    Full,
}

#[derive(Clone, Debug)]
pub(crate) struct RenderRuntimePolicy {
    pub render_phase_log: bool,
    pub primary_ui_enabled: bool,
    pub primitive_stage_log: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SimulationRuntimePolicy {
    pub telemetry_interval_ticks: u64,
    pub slow_tick_ms: f32,
    pub physics_stage_log: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DiagnosticsPolicy {
    pub render_trace_ms: f32,
    pub render_warn_ms: f32,
    pub render_profiler_outlier_ms: f32,
    pub render_slow_profile_interval_frames: u64,
    pub render_profiler_sample_interval_frames: u64,
    pub render_profiler_samples: bool,
    pub render_job_event_mode: RenderJobEventMode,
    pub render_job_event_interval_frames: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct StreamingPolicy {
    pub terrain_gpu_uploads_per_frame: u32,
    pub primitive_gpu_uploads_per_frame: u32,
    pub model_render_prep_jobs: u32,
    pub model_gpu_uploads_per_frame: u32,
    pub primitive_gpu_upload_warn_ms: f32,
    pub terrain_gpu_upload_interval_frames: u64,
    pub scene_texture_gate_soft_timeout_frames: u64,
    pub scene_texture_gate_soft_timeout_ms: u64,
    pub scene_texture_launch_min_ready: Option<u32>,
    pub scene_texture_launch_min_ratio: f32,
    pub terrain_launch_min_ready_packets: u32,
}

static RENDER: OnceLock<RenderRuntimePolicy> = OnceLock::new();
static SIMULATION: OnceLock<SimulationRuntimePolicy> = OnceLock::new();
static DIAGNOSTICS: OnceLock<DiagnosticsPolicy> = OnceLock::new();
static STREAMING: OnceLock<StreamingPolicy> = OnceLock::new();

#[inline]
pub(crate) fn render_runtime_policy() -> &'static RenderRuntimePolicy {
    RENDER.get_or_init(|| RenderRuntimePolicy {
        render_phase_log: crate::env_config::var_bool("NEWENGINE_RENDER_PHASE_LOG", false),
        primary_ui_enabled: crate::env_config::var_bool("NEWENGINE_PRIMARY_UI_ENABLED", false),
        primitive_stage_log: crate::env_config::var_bool("NEWENGINE_PRIMITIVE_STAGE_LOG", false),
    })
}

#[inline]
pub(crate) fn simulation_runtime_policy() -> &'static SimulationRuntimePolicy {
    SIMULATION.get_or_init(|| SimulationRuntimePolicy {
        telemetry_interval_ticks: crate::env_config::var_u64(
            "NEWENGINE_SIM_TELEMETRY_INTERVAL_TICKS",
            120,
            1,
            60_000,
        ),
        slow_tick_ms: crate::env_config::var_f32("NEWENGINE_SIM_SLOW_TICK_MS", 4.0, 0.25, 1000.0),
        physics_stage_log: crate::env_config::var_bool("NEWENGINE_PHYSICS_STAGE_LOG", false),
    })
}

#[inline]
pub(crate) fn diagnostics_policy() -> &'static DiagnosticsPolicy {
    DIAGNOSTICS.get_or_init(|| {
        let mode = crate::env_config::var("NEWENGINE_RENDER_JOB_EVENT_MODE")
            .unwrap_or_else(|| "off".to_owned());
        let render_job_event_mode = match mode.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "disabled" => RenderJobEventMode::Off,
            "full" | "all" | "trace" => RenderJobEventMode::Full,
            _ => RenderJobEventMode::Sampled,
        };
        DiagnosticsPolicy {
            render_trace_ms: crate::env_config::var_f32(
                "NEWENGINE_RENDER_TRACE_MS",
                16.67,
                1.0,
                1000.0,
            ),
            render_warn_ms: crate::env_config::var_f32(
                "NEWENGINE_RENDER_WARN_MS",
                16.67,
                1.0,
                2000.0,
            ),
            render_profiler_outlier_ms: crate::env_config::var_f32(
                "NEWENGINE_RENDER_PROFILER_OUTLIER_MS",
                8.0,
                1.0,
                2000.0,
            ),
            render_slow_profile_interval_frames: crate::env_config::var_u64(
                "NEWENGINE_RENDER_SLOW_PROFILE_INTERVAL_FRAMES",
                120,
                1,
                6000,
            ),
            render_profiler_sample_interval_frames: crate::env_config::var_u64(
                "NEWENGINE_RENDER_PROFILER_SAMPLE_INTERVAL_FRAMES",
                120,
                1,
                6000,
            ),
            render_profiler_samples: crate::env_config::var_bool(
                "NEWENGINE_RENDER_PROFILER_SAMPLES",
                true,
            ),
            render_job_event_mode,
            render_job_event_interval_frames: crate::env_config::var_u64(
                "NEWENGINE_RENDER_JOB_EVENT_INTERVAL_FRAMES",
                120,
                1,
                6000,
            ),
        }
    })
}

#[inline]
pub(crate) fn streaming_policy() -> &'static StreamingPolicy {
    STREAMING.get_or_init(|| StreamingPolicy {
        terrain_gpu_uploads_per_frame: crate::env_config::var_u32(
            "NEWENGINE_TERRAIN_GPU_UPLOADS_PER_FRAME",
            8,
            0,
            32,
        ),
        primitive_gpu_uploads_per_frame: crate::env_config::var_u32(
            "NEWENGINE_PRIMITIVE_GPU_UPLOADS_PER_FRAME",
            1,
            0,
            8,
        ),
        model_render_prep_jobs: crate::env_config::var_u32(
            "NEWENGINE_MODEL_RENDER_PREP_JOBS",
            2,
            1,
            16,
        ),
        model_gpu_uploads_per_frame: crate::env_config::var_u32(
            "NEWENGINE_MODEL_GPU_UPLOADS_PER_FRAME",
            1,
            0,
            8,
        ),
        primitive_gpu_upload_warn_ms: crate::env_config::var_f32(
            "NEWENGINE_PRIMITIVE_GPU_UPLOAD_WARN_MS",
            250.0,
            16.0,
            5000.0,
        ),
        terrain_gpu_upload_interval_frames: crate::env_config::var_u64(
            "NEWENGINE_TERRAIN_GPU_UPLOAD_INTERVAL_FRAMES",
            1,
            1,
            240,
        ),
        scene_texture_gate_soft_timeout_frames: crate::env_config::var_u64(
            "NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES",
            1_800,
            60,
            18_000,
        ),
        scene_texture_gate_soft_timeout_ms: crate::env_config::var_u64(
            "NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_MS",
            90_000,
            5_000,
            600_000,
        ),
        scene_texture_launch_min_ready: crate::env_config::var(
            "NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY",
        )
        .and_then(|value| value.trim().parse::<u32>().ok()),
        scene_texture_launch_min_ratio: crate::env_config::var_f32(
            "NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO",
            1.00,
            0.50,
            1.00,
        ),
        terrain_launch_min_ready_packets: crate::env_config::var_u32(
            "NEWENGINE_TERRAIN_LAUNCH_MIN_READY_PACKETS",
            1,
            1,
            64,
        ),
    })
}
