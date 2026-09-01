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
        RenderHardwareTier::LegacyGtx => 2,
        RenderHardwareTier::Gtx => 4,
        RenderHardwareTier::Rtx => 8,
        RenderHardwareTier::Unknown => 4,
    };
    cpu_cap.min(tier_cap).max(1)
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
        assert!(material_texture_async_decode_ceiling(RenderHardwareTier::LegacyGtx) <= 2);
        assert!(material_texture_async_decode_ceiling(RenderHardwareTier::Gtx) <= 4);
        assert!(material_texture_async_decode_ceiling(RenderHardwareTier::Rtx) <= 8);
        assert!(material_texture_async_decode_ceiling(RenderHardwareTier::Rtx) >= 1);
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
