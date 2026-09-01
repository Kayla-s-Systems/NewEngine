#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    Extent2D, RenderApi, RenderTargetDesc, RenderTargetId, TextureFormat, TextureId,
};
use newengine_core::EngineResult;

use crate::render_controller::RuntimeRenderController;

#[inline]
fn shadow_rt_extent_key(extent: Extent2D) -> u32 {
    let w = extent.width.min(0xFFFF);
    let h = extent.height.min(0xFFFF);
    (w << 16) | h
}

#[inline]
fn shadow_atlas_grid(cascade_count: u32) -> (u32, u32) {
    let cascades = cascade_count.clamp(1, 4);
    let columns = if cascades <= 1 { 1 } else { 2 };
    let rows = cascades.div_ceil(columns).max(1);
    (columns, rows)
}

#[inline]
fn effective_shadow_tile_resolution(
    requested_resolution: u32,
    cascade_count: u32,
    max_texture_dimension_2d: u32,
) -> u32 {
    let requested = requested_resolution.clamp(
        super::super::super::render_quality::SHADOW_RESOLUTION_MIN,
        super::super::super::render_quality::SHADOW_RESOLUTION_MAX,
    );
    let (columns, rows) = shadow_atlas_grid(cascade_count);
    let max_dimension =
        max_texture_dimension_2d.max(super::super::super::render_quality::SHADOW_RESOLUTION_MIN);
    let tile_limit = (max_dimension / columns)
        .min(max_dimension / rows)
        .max(super::super::super::render_quality::SHADOW_RESOLUTION_MIN);
    requested.min(tile_limit)
}

#[inline]
fn next_shadow_resolution_fallback(current: u32) -> Option<u32> {
    [8192_u32, 4096, 2048, 1024, 512, 256]
        .into_iter()
        .find(|&candidate| candidate < current)
}

#[inline]
pub(super) fn ensure_shadow_rt(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    requested_resolution: u32,
    cascade_count: u32,
) -> EngineResult<Option<(RenderTargetId, TextureId, u32)>> {
    let requested = requested_resolution.clamp(
        super::super::super::render_quality::SHADOW_RESOLUTION_MIN,
        super::super::super::render_quality::SHADOW_RESOLUTION_MAX,
    );
    let cascades = cascade_count.clamp(1, 4);
    let desired_resolution = effective_shadow_tile_resolution(
        requested,
        cascades,
        this.shadows.max_texture_dimension_2d,
    );

    let request_changed = this.shadows.render_target.is_none()
        || this.shadows.render_target_requested_resolution != requested
        || this.shadows.render_target_cascade_count != cascades
        || this.shadows.render_target_tile_resolution > desired_resolution;

    if request_changed {
        if let Some(old) = this.shadows.render_target.take() {
            this.retire_render_target_with_reason(old, "shadow_target_reconfigured");
        }
        this.shadows.render_target_resolution = 0;
        this.shadows.render_target_tile_resolution = 0;
        this.shadows.render_target_requested_resolution = requested;
        this.shadows.render_target_cascade_count = cascades;
        this.invalidate_shadow_cache();

        let (columns, rows) = shadow_atlas_grid(cascades);
        let mut candidate = desired_resolution;
        loop {
            let atlas_extent = Extent2D::new(
                candidate.saturating_mul(columns),
                candidate.saturating_mul(rows),
            );
            let label = if cascades > 1 {
                format!(
                    "game_sun_csm_atlas_{}x{}_cascades_{}_tile_{}",
                    atlas_extent.width, atlas_extent.height, cascades, candidate
                )
            } else {
                format!("game_sun_shadow_map_{candidate}")
            };

            match r.create_render_target(
                RenderTargetDesc::new(
                    atlas_extent,
                    super::super::super::render_quality::SHADOW_MAP_COLOR_FORMAT,
                )
                .with_depth(TextureFormat::Depth32Float)
                .with_label(label),
            ) {
                Ok(rt) => {
                    this.shadows.render_target = Some(rt);
                    this.shadows.render_target_resolution = shadow_rt_extent_key(atlas_extent);
                    this.shadows.render_target_tile_resolution = candidate;
                    if candidate != requested {
                        newengine_ulog_api::ulog::warn!(
                            "render shadows: requested tile={} cascades={} resolved tile={} atlas={}x{} gpu_max_2d={}; keeping shadows active with safe effective resolution",
                            requested,
                            cascades,
                            candidate,
                            atlas_extent.width,
                            atlas_extent.height,
                            this.shadows.max_texture_dimension_2d,
                        );
                    }
                    break;
                }
                Err(error) => {
                    let Some(next) = next_shadow_resolution_fallback(candidate) else {
                        return Err(error);
                    };
                    newengine_ulog_api::ulog::warn!(
                        "render shadows: allocation failed tile={} cascades={} gpu_max_2d={} ({error}); retrying tile={}",
                        candidate,
                        cascades,
                        this.shadows.max_texture_dimension_2d,
                        next,
                    );
                    candidate = next;
                }
            }
        }
    }

    let Some(rt) = this.shadows.render_target else {
        return Ok(None);
    };
    let tex = r.render_target_color_texture_id(rt)?;
    Ok(Some((
        rt,
        tex,
        this.shadows.render_target_tile_resolution.max(1),
    )))
}

#[inline]
pub(super) fn ensure_local_shadow_rt(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    requested_extent: Extent2D,
) -> EngineResult<Option<(RenderTargetId, TextureId)>> {
    let extent = Extent2D::new(
        requested_extent.width.clamp(128, 8192),
        requested_extent.height.clamp(128, 8192),
    );
    let extent_key = shadow_rt_extent_key(extent);
    let recreate = this.shadows.local_render_target.is_none()
        || this.shadows.local_render_target_extent_key != extent_key;
    if recreate {
        if let Some(old) = this.shadows.local_render_target.take() {
            this.retire_render_target_with_reason(old, "shadow_target_reconfigured");
        }
        this.shadows.local_render_target_extent_key = 0;
        this.invalidate_local_shadow_cache();
        let rt = r.create_render_target(
            RenderTargetDesc::new(
                extent,
                super::super::super::render_quality::SHADOW_MAP_COLOR_FORMAT,
            )
            .with_depth(TextureFormat::Depth32Float)
            .with_label(format!(
                "game_local_shadow_atlas_{}x{}",
                extent.width, extent.height
            )),
        )?;
        this.shadows.local_render_target = Some(rt);
        this.shadows.local_render_target_extent_key = extent_key;
    }
    let Some(rt) = this.shadows.local_render_target else {
        return Ok(None);
    };
    let texture = r.render_target_color_texture_id(rt)?;
    Ok(Some((rt, texture)))
}

#[inline]
pub(super) fn retire_local_shadow_rt(this: &mut RuntimeRenderController) {
    if let Some(old) = this.shadows.local_render_target.take() {
        this.retire_render_target_with_reason(old, "shadow_target_reconfigured");
    }
    this.shadows.local_render_target_extent_key = 0;
    this.invalidate_local_shadow_cache();
}

#[inline]
pub(super) fn retire_shadow_rt(this: &mut RuntimeRenderController) {
    if let Some(old) = this.shadows.render_target.take() {
        this.retire_render_target_with_reason(old, "shadow_target_reconfigured");
    }
    this.shadows.render_target_resolution = 0;
    this.shadows.render_target_tile_resolution = 0;
    this.shadows.render_target_requested_resolution = 0;
    this.shadows.render_target_cascade_count = 0;
    this.invalidate_shadow_cache();
}

#[inline]
pub(super) fn warn_unsupported_point_shadow_once(this: &mut RuntimeRenderController) {
    if this.shadows.unsupported_point_warning_emitted {
        return;
    }
    this.shadows.unsupported_point_warning_emitted = true;
    newengine_ulog_api::ulog::warn!(
        "render shadows: PointLight is shadow-capable, but point cube-map shadows are not implemented by this backend path yet; falling back to unshadowed point lighting"
    );
}

#[inline]
pub(super) fn warn_unsupported_spot_shadow_once(this: &mut RuntimeRenderController) {
    if this.shadows.unsupported_spot_warning_emitted {
        return;
    }
    this.shadows.unsupported_spot_warning_emitted = true;
    newengine_ulog_api::ulog::warn!(
        "render shadows: Spot shadow maps are planned, but no SpotLight component/backend path is implemented yet; falling back to unshadowed lighting"
    );
}

#[cfg(test)]
mod directional_shadow_target_tests {
    use super::*;

    #[test]
    fn effective_shadow_tile_resolution_respects_full_atlas_limit() {
        assert_eq!(effective_shadow_tile_resolution(8192, 4, 16384), 8192);
        assert_eq!(effective_shadow_tile_resolution(8192, 4, 8192), 4096);
        assert_eq!(effective_shadow_tile_resolution(16284, 1, 16384), 16284);
        assert_eq!(effective_shadow_tile_resolution(16284, 4, 16384), 8192);
    }

    #[test]
    fn shadow_allocation_fallback_never_increases_resolution() {
        assert_eq!(next_shadow_resolution_fallback(16284), Some(8192));
        assert_eq!(next_shadow_resolution_fallback(8192), Some(4096));
        assert_eq!(next_shadow_resolution_fallback(256), None);
    }
}
