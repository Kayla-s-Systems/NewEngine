#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::TextureFormat;

/// Runtime renderer quality constants for the current Vulkan forward path.
///
/// This is deliberately a small declarative profile instead of scattering magic
/// values through shadow planning, shader setup and scene tuning. The target
/// architecture is a full RenderSettings/quality-profile resource, but constants
/// here keep this pass safe and compile-local.
pub(crate) const SCENE_HDR_COLOR_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub(crate) const SHADOW_MAP_COLOR_FORMAT: TextureFormat = TextureFormat::R32Float;
pub(crate) const SHADOW_STRENGTH_MAX: f32 = 0.82;
pub(crate) const SHADOW_SOFTNESS_MAX: f32 = 1.25;
pub(crate) const SHADOW_RESOLUTION_MIN: u32 = 256;
pub(crate) const SHADOW_RESOLUTION_MAX: u32 = 4096;

/// Loading-screen budget for starting material texture decode jobs.
///
/// `assets.textures.entry_runtime_v1` is a heavy semantic texture operation.
/// Render may enqueue/poll this work, but it must not synchronously decode a
/// dictionary on the present/submit thread.
pub(crate) const MATERIAL_TEXTURE_IMPORT_START_BURST: u32 = 4;
pub(crate) const MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS: f32 = 2.0;
/// Maximum in-flight material texture decode jobs submitted to engine.jobs.
///
/// `call_service_v1` stays synchronous inside a worker job, but the render
/// thread no longer blocks on the asset provider. This is the first hot-path
/// rule: heavy service work is ticketed and polled, not awaited by frame submit.
///
/// Defaults intentionally bias toward fast startup/world reveal: several
/// decode jobs may be in flight at once, but the frame thread still only pumps
/// completion/residency work within explicit budgets.
pub(crate) const MATERIAL_TEXTURE_MAX_ASYNC_DECODE_JOBS: usize = 6;

/// The first playable frames should present quickly; expensive shadow cache
/// population is staged immediately after initial visibility and material
/// bindings are warm. This mirrors the reference renderer's phased draw-list
/// warmup instead of doing every heavy pass on frame one.
pub(crate) const SHADOW_WARMUP_DEFER_FRAMES: u8 = 0;


#[inline]
pub(crate) const fn shadow_refresh_period_frames() -> u64 {
    // Stable CSM should not refresh every frame by default. Runtime overrides
    // now live in the declarative render runtime profile instead of environment
    // variables embedded in engine code.
    12
}
