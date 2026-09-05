#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RenderHardwareTier, TextureFormat};

/// Runtime renderer quality constants for the current backend forward path.
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
pub(crate) const SHADOW_RESOLUTION_MAX: u32 = 16284;

/// Inputs that decide which primary lit pipeline variant a composition needs.
///
/// The loading-frame prewarm path and the live submit path MUST resolve the same
/// variant. Warming a pipeline the frame path will not bind spends the entire
/// warmup budget on a dead cache entry and defers the real shader compile into
/// live rendering, which shows up as a second warmup wave once the world is
/// playable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScenePipelineVariantInputs {
    /// `RuntimeProfile::hdr_scene_enabled` as requested by the profile.
    pub hdr_scene_requested: bool,
    /// `RuntimeProfile::postfx_enabled` as requested by the profile.
    pub postfx_requested: bool,
    /// `RuntimeProfile::deferred_enabled` as requested by the profile.
    pub deferred_requested: bool,
    /// An external preview target owns the presented image.
    pub external_preview_target: bool,
    /// The editor viewport is driving this composition.
    pub editor_active: bool,
    /// The editor viewport requests a non-lit debug shading mode.
    pub editor_debug_shading: bool,
    /// The frame renders straight into the window surface instead of an offscreen RT.
    pub direct_surface_viewport: bool,
}

/// Primary lit pipeline variant resolved from [`ScenePipelineVariantInputs`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScenePipelineVariant {
    /// Render-pass color format the pipeline must be baked against.
    pub scene_color_format: TextureFormat,
    /// Effective deferred-path state after preview/editor policy is applied.
    pub deferred_enabled: bool,
}

/// Single authority for primary lit pipeline variant selection.
pub(crate) fn resolve_scene_pipeline_variant(
    inputs: ScenePipelineVariantInputs,
) -> ScenePipelineVariant {
    let hdr_scene_enabled =
        inputs.hdr_scene_requested && !inputs.external_preview_target && !inputs.editor_active;
    let postfx_enabled =
        inputs.postfx_requested && !inputs.external_preview_target && !inputs.editor_debug_shading;
    let deferred_enabled = inputs.deferred_requested
        && !inputs.external_preview_target
        && !inputs.editor_debug_shading;
    let scene_offscreen = hdr_scene_enabled || postfx_enabled;
    let scene_color_format = if hdr_scene_enabled {
        SCENE_HDR_COLOR_FORMAT
    } else if inputs.direct_surface_viewport && !scene_offscreen {
        // The Vulkan WSI contract is BGRA8_SRGB. A direct-to-surface LDR material
        // pipeline must be baked against that exact render-pass format; offscreen
        // LDR targets stay UNORM so they remain sampleable linear intermediates.
        TextureFormat::Bgra8Srgb
    } else {
        TextureFormat::Bgra8Unorm
    };
    ScenePipelineVariant {
        scene_color_format,
        deferred_enabled,
    }
}

/// Loading-screen budget for starting material texture decode jobs.
///
/// `assets.textures.entry_runtime_v1` is now resolved through StarVault + the
/// registered YTD format descriptor/ListFile codec. There is no longer a single
/// mutable `engine.assets.textures` provider mutex serializing all dictionary
/// work, so the loading gate may keep several independent decode jobs in flight.
pub(crate) const MATERIAL_TEXTURE_IMPORT_START_BURST: u32 = 4;
pub(crate) const MATERIAL_TEXTURE_DECODE_PUMP_BUDGET_MS: f32 = 2.0;
/// Maximum in-flight material texture decode jobs submitted to engine.threading.
///
/// Four jobs keeps the loading screen responsive while allowing static-world
/// textures discovered after character/model materials to catch up immediately.
pub(crate) const MATERIAL_TEXTURE_MAX_ASYNC_DECODE_JOBS: usize = 4;

/// Adaptive CPU decode ceiling. This is independent from the GPU upload budget: decoding may run
/// ahead on worker threads while uploads remain byte/job bounded per frame.
pub(crate) fn material_texture_async_decode_ceiling(tier: RenderHardwareTier) -> usize {
    let cpu = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4);
    let cpu_cap = cpu.saturating_sub(2).clamp(1, 16);
    let tier_cap = match tier {
        RenderHardwareTier::Headless => 1,
        RenderHardwareTier::LegacyGtx => 1,
        RenderHardwareTier::Gtx => 2,
        RenderHardwareTier::Rtx => 4,
        RenderHardwareTier::Unknown => 2,
    };
    cpu_cap.min(tier_cap).max(1)
}

/// Playable-frame decode admission is intentionally stricter than loading-screen admission.
/// Once the scene is interactive, long-running semantic texture decodes are background work and
/// must leave CPU headroom for fixed simulation, animation and render preparation.
///
/// This is a concurrency ceiling, not a throughput cap: the queue keeps its priority ordering and
/// immediately admits the next texture when the previous decode completes.
pub(crate) fn material_texture_playable_async_decode_ceiling(tier: RenderHardwareTier) -> usize {
    match tier {
        RenderHardwareTier::Headless | RenderHardwareTier::LegacyGtx | RenderHardwareTier::Gtx => 1,
        RenderHardwareTier::Rtx => 2,
        RenderHardwareTier::Unknown => 1,
    }
}

/// Number of new background texture decode jobs admitted from a playable frame.
/// Completion harvest and GPU upload remain active even when catch-up suppresses new CPU work.
#[inline]
pub(crate) fn material_texture_playable_start_budget(
    tier: RenderHardwareTier,
    configured_jobs: u32,
    fixed_step_count: u32,
) -> u32 {
    if fixed_step_count > 1 {
        return 0;
    }
    configured_jobs.min(material_texture_playable_async_decode_ceiling(tier) as u32)
}
/// Hard safety boundary for a single service-to-renderer texture allocation.
/// Assets above this threshold must be block-compressed or reduced during import;
/// the runtime uses shader fallbacks rather than freezing the native window.
pub(crate) const MATERIAL_TEXTURE_MAX_UPLOAD_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
/// Upper guard for upload count; the effective count/byte budget is hardware-tier aware below.
pub(crate) const MATERIAL_TEXTURE_MAX_UPLOADS_PER_FRAME: u32 = 4;

/// GPU texture upload budget is intentionally independent from CPU decode concurrency.
/// Decoded packets may accumulate in the renderer-owned upload queue while the render thread
/// admits only a bounded amount of VRAM transfer work per frame.
pub(crate) fn material_texture_gpu_upload_budget(tier: RenderHardwareTier) -> (u32, usize) {
    const MIB: usize = 1024 * 1024;
    match tier {
        RenderHardwareTier::Headless => (0, 0),
        RenderHardwareTier::LegacyGtx => (1, 4 * MIB),
        RenderHardwareTier::Gtx => (2, 12 * MIB),
        RenderHardwareTier::Rtx => (4, 32 * MIB),
        RenderHardwareTier::Unknown => (2, 8 * MIB),
    }
}

/// The first playable frames should present quickly; expensive shadow cache
/// population is staged immediately after initial visibility and material
/// bindings are warm. This mirrors the reference renderer's phased draw-list
/// warmup instead of doing every heavy pass on frame one.
pub(crate) const SHADOW_WARMUP_DEFER_FRAMES: u8 = 0;

#[cfg(test)]
mod texture_streaming_quality_tests {
    use super::*;

    #[test]
    fn adaptive_decode_ceiling_respects_hardware_tier() {
        assert_eq!(
            material_texture_async_decode_ceiling(RenderHardwareTier::Headless),
            1
        );
        assert_eq!(
            material_texture_async_decode_ceiling(RenderHardwareTier::LegacyGtx),
            1
        );
        assert!(material_texture_async_decode_ceiling(RenderHardwareTier::Gtx) <= 2);
        assert!(material_texture_async_decode_ceiling(RenderHardwareTier::Rtx) <= 4);
        assert!(material_texture_async_decode_ceiling(RenderHardwareTier::Rtx) >= 1);
    }

    #[test]
    fn playable_decode_ceiling_reserves_cpu_headroom() {
        assert_eq!(
            material_texture_playable_async_decode_ceiling(RenderHardwareTier::LegacyGtx),
            1
        );
        assert_eq!(
            material_texture_playable_async_decode_ceiling(RenderHardwareTier::Gtx),
            1
        );
        assert!(
            material_texture_playable_async_decode_ceiling(RenderHardwareTier::Rtx)
                <= material_texture_async_decode_ceiling(RenderHardwareTier::Rtx)
        );
    }

    #[test]
    fn catch_up_frame_suppresses_new_background_decode_admission() {
        assert_eq!(
            material_texture_playable_start_budget(RenderHardwareTier::Gtx, 4, 2),
            0
        );
        assert_eq!(
            material_texture_playable_start_budget(RenderHardwareTier::Gtx, 4, 4),
            0
        );
        assert_eq!(
            material_texture_playable_start_budget(RenderHardwareTier::Gtx, 4, 1),
            1
        );
        assert_eq!(
            material_texture_playable_start_budget(RenderHardwareTier::Rtx, 4, 1),
            2
        );
    }

    #[test]
    fn gpu_upload_budget_is_separate_and_tier_bounded() {
        assert_eq!(
            material_texture_gpu_upload_budget(RenderHardwareTier::Headless),
            (0, 0)
        );
        let legacy = material_texture_gpu_upload_budget(RenderHardwareTier::LegacyGtx);
        let gtx = material_texture_gpu_upload_budget(RenderHardwareTier::Gtx);
        let rtx = material_texture_gpu_upload_budget(RenderHardwareTier::Rtx);
        assert!(legacy.0 <= gtx.0 && gtx.0 <= rtx.0);
        assert!(legacy.1 < gtx.1 && gtx.1 < rtx.1);
        assert!(rtx.0 <= MATERIAL_TEXTURE_MAX_UPLOADS_PER_FRAME);
    }
}
