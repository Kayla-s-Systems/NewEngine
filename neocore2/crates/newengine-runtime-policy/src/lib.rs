#![forbid(unsafe_op_in_unsafe_fn)]

//! Immutable startup policy snapshots for frame-critical runtime systems.
//!
//! Process environment is a bootstrap/config compatibility boundary, not a frame
//! API. Values in this module are normalized once and then consumed as typed
//! policy by render, simulation, diagnostics and streaming paths.

use std::sync::OnceLock;
mod env {
    #[inline]
    pub fn var(name: &str) -> Option<String> {
        newengine_plugin_host::current_host_context().environment_var(name)
    }

    #[inline]
    pub fn var_bool(name: &str, default: bool) -> bool {
        var(name)
            .map(|value| {
                let value = value.trim().to_ascii_lowercase();
                !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
            })
            .unwrap_or(default)
    }

    #[inline]
    pub fn var_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
        var(name)
            .and_then(|value| value.trim().parse::<f32>().ok())
            .map(|value| value.clamp(min, max))
            .unwrap_or(default)
    }

    #[inline]
    pub fn var_u32(name: &str, default: u32, min: u32, max: u32) -> u32 {
        var(name)
            .and_then(|value| value.trim().parse::<u32>().ok())
            .map(|value| value.clamp(min, max))
            .unwrap_or(default)
    }

    #[inline]
    pub fn var_u64(name: &str, default: u64, min: u64, max: u64) -> u64 {
        var(name)
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(|value| value.clamp(min, max))
            .unwrap_or(default)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderJobEventMode {
    Off,
    Sampled,
    Full,
}

#[derive(Clone, Debug)]
pub struct RenderRuntimePolicy {
    pub render_phase_log: bool,
    pub primary_ui_enabled: bool,
    pub primitive_stage_log: bool,
    pub render_route_diagnostics: bool,
    pub render_route_diagnostic_interval_frames: u64,
}

#[derive(Clone, Debug)]
pub struct SimulationRuntimePolicy {
    pub telemetry_interval_ticks: u64,
    pub slow_tick_ms: f32,
    pub physics_stage_log: bool,
}

#[derive(Clone, Debug)]
pub struct DiagnosticsPolicy {
    pub render_trace_ms: f32,
    pub render_warn_ms: f32,
    pub render_profiler_outlier_ms: f32,
    pub render_slow_profile_interval_frames: u64,
    pub render_profiler_sample_interval_frames: u64,
    pub render_profiler_samples: bool,
    pub render_steady_trace_interval_frames: u64,
    pub render_job_event_mode: RenderJobEventMode,
    pub render_job_event_interval_frames: u64,
}

#[derive(Clone, Debug)]
pub struct StreamingPolicy {
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
pub fn render_runtime_policy() -> &'static RenderRuntimePolicy {
    RENDER.get_or_init(|| RenderRuntimePolicy {
        render_phase_log: env::var_bool("NEWENGINE_RENDER_PHASE_LOG", false),
        primary_ui_enabled: env::var_bool("NEWENGINE_PRIMARY_UI_ENABLED", false),
        primitive_stage_log: env::var_bool("NEWENGINE_PRIMITIVE_STAGE_LOG", false),
        render_route_diagnostics: env::var_bool("NEWENGINE_RENDER_ROUTE_DIAGNOSTICS", false),
        render_route_diagnostic_interval_frames: env::var_u64(
            "NEWENGINE_RENDER_ROUTE_DIAGNOSTIC_INTERVAL_FRAMES",
            240,
            1,
            60_000,
        ),
    })
}

#[inline]
pub fn simulation_runtime_policy() -> &'static SimulationRuntimePolicy {
    SIMULATION.get_or_init(|| SimulationRuntimePolicy {
        telemetry_interval_ticks: env::var_u64(
            "NEWENGINE_SIM_TELEMETRY_INTERVAL_TICKS",
            120,
            1,
            60_000,
        ),
        slow_tick_ms: env::var_f32("NEWENGINE_SIM_SLOW_TICK_MS", 4.0, 0.25, 1000.0),
        physics_stage_log: env::var_bool("NEWENGINE_PHYSICS_STAGE_LOG", false),
    })
}

#[inline]
pub fn diagnostics_policy() -> &'static DiagnosticsPolicy {
    DIAGNOSTICS.get_or_init(|| {
        let mode = env::var("NEWENGINE_RENDER_JOB_EVENT_MODE").unwrap_or_else(|| "off".to_owned());
        let render_job_event_mode = match mode.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "disabled" => RenderJobEventMode::Off,
            "full" | "all" | "trace" => RenderJobEventMode::Full,
            _ => RenderJobEventMode::Sampled,
        };
        DiagnosticsPolicy {
            render_trace_ms: env::var_f32("NEWENGINE_RENDER_TRACE_MS", 16.67, 1.0, 1000.0),
            render_warn_ms: env::var_f32("NEWENGINE_RENDER_WARN_MS", 16.67, 1.0, 2000.0),
            // Outlier capture must not make a healthy 60 Hz render frame a synchronous
            // profiler event. Sample budget misses immediately; steady frames use the sparse
            // interval below. This keeps observability off the critical path by default.
            render_profiler_outlier_ms: env::var_f32(
                "NEWENGINE_RENDER_PROFILER_OUTLIER_MS",
                16.67,
                1.0,
                2000.0,
            ),
            render_slow_profile_interval_frames: env::var_u64(
                "NEWENGINE_RENDER_SLOW_PROFILE_INTERVAL_FRAMES",
                120,
                1,
                6000,
            ),
            render_profiler_sample_interval_frames: env::var_u64(
                "NEWENGINE_RENDER_PROFILER_SAMPLE_INTERVAL_FRAMES",
                120,
                1,
                6000,
            ),
            render_profiler_samples: env::var_bool("NEWENGINE_RENDER_PROFILER_SAMPLES", true),
            // Human-readable steady-state trace output is opt-in. A synchronous
            // console/file sink must not introduce deterministic frame hitches.
            render_steady_trace_interval_frames: env::var_u64(
                "NEWENGINE_RENDER_STEADY_TRACE_INTERVAL_FRAMES",
                0,
                0,
                60_000,
            ),
            render_job_event_mode,
            render_job_event_interval_frames: env::var_u64(
                "NEWENGINE_RENDER_JOB_EVENT_INTERVAL_FRAMES",
                120,
                1,
                6000,
            ),
        }
    })
}

#[inline]
pub fn streaming_policy() -> &'static StreamingPolicy {
    STREAMING.get_or_init(|| StreamingPolicy {
        terrain_gpu_uploads_per_frame: env::var_u32(
            "NEWENGINE_TERRAIN_GPU_UPLOADS_PER_FRAME",
            8,
            0,
            32,
        ),
        primitive_gpu_uploads_per_frame: env::var_u32(
            "NEWENGINE_PRIMITIVE_GPU_UPLOADS_PER_FRAME",
            1,
            0,
            8,
        ),
        model_render_prep_jobs: env::var_u32("NEWENGINE_MODEL_RENDER_PREP_JOBS", 2, 1, 16),
        model_gpu_uploads_per_frame: env::var_u32("NEWENGINE_MODEL_GPU_UPLOADS_PER_FRAME", 1, 0, 8),
        primitive_gpu_upload_warn_ms: env::var_f32(
            "NEWENGINE_PRIMITIVE_GPU_UPLOAD_WARN_MS",
            250.0,
            16.0,
            5000.0,
        ),
        terrain_gpu_upload_interval_frames: env::var_u64(
            "NEWENGINE_TERRAIN_GPU_UPLOAD_INTERVAL_FRAMES",
            1,
            1,
            240,
        ),
        scene_texture_gate_soft_timeout_frames: env::var_u64(
            "NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_FRAMES",
            1_800,
            60,
            18_000,
        ),
        scene_texture_gate_soft_timeout_ms: env::var_u64(
            "NEWENGINE_SCENE_TEXTURE_GATE_SOFT_TIMEOUT_MS",
            90_000,
            5_000,
            600_000,
        ),
        scene_texture_launch_min_ready: env::var("NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY")
            .and_then(|value| value.trim().parse::<u32>().ok()),
        scene_texture_launch_min_ratio: env::var_f32(
            "NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO",
            1.00,
            0.50,
            1.00,
        ),
        terrain_launch_min_ready_packets: env::var_u32(
            "NEWENGINE_TERRAIN_LAUNCH_MIN_READY_PACKETS",
            1,
            1,
            64,
        ),
    })
}
