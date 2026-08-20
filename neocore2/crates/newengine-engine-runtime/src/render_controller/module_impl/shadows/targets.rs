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
pub(super) fn ensure_shadow_rt(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    requested_resolution: u32,
    cascade_count: u32,
) -> EngineResult<Option<(RenderTargetId, TextureId)>> {
    let resolution = requested_resolution.clamp(
        super::super::super::render_quality::SHADOW_RESOLUTION_MIN,
        super::super::super::render_quality::SHADOW_RESOLUTION_MAX,
    );
    let cascades = cascade_count.clamp(1, 4);
    let columns = if cascades <= 1 { 1 } else { 2 };
    let rows = cascades.div_ceil(columns).max(1);
    let atlas_extent = Extent2D::new(
        resolution.saturating_mul(columns),
        resolution.saturating_mul(rows),
    );
    let atlas_key = shadow_rt_extent_key(atlas_extent);
    let recreate =
        this.shadows.render_target.is_none() || this.shadows.render_target_resolution != atlas_key;

    if recreate {
        if let Some(old) = this.shadows.render_target.take() {
            this.retire_render_target(old);
        }
        this.shadows.render_target_resolution = 0;
        this.invalidate_shadow_cache();
        let rt = r.create_render_target(
            RenderTargetDesc::new(
                atlas_extent,
                super::super::super::render_quality::SHADOW_MAP_COLOR_FORMAT,
            )
            .with_depth(TextureFormat::Depth32Float)
            .with_label(if cascades > 1 {
                format!(
                    "game_sun_csm_atlas_{}x{}_cascades_{}",
                    atlas_extent.width, atlas_extent.height, cascades
                )
            } else {
                format!("game_sun_shadow_map_{resolution}")
            }),
        )?;
        this.shadows.render_target = Some(rt);
        this.shadows.render_target_resolution = atlas_key;
    }

    let Some(rt) = this.shadows.render_target else {
        return Ok(None);
    };
    let tex = r.render_target_color_texture_id(rt)?;
    Ok(Some((rt, tex)))
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
            this.retire_render_target(old);
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
        this.retire_render_target(old);
    }
    this.shadows.local_render_target_extent_key = 0;
    this.invalidate_local_shadow_cache();
}

#[inline]
pub(super) fn retire_shadow_rt(this: &mut RuntimeRenderController) {
    if let Some(old) = this.shadows.render_target.take() {
        this.retire_render_target(old);
    }
    this.shadows.render_target_resolution = 0;
    this.invalidate_shadow_cache();
}

#[inline]
pub(super) fn warn_unsupported_point_shadow_once(this: &mut RuntimeRenderController) {
    if this.shadows.unsupported_point_warning_emitted {
        return;
    }
    this.shadows.unsupported_point_warning_emitted = true;
    newengine_ulog_api::ulog::warn!(
        "render shadows: PointLight is shadow-capable, but point cube-map shadows are not implemented by this Vulkan path yet; falling back to unshadowed point lighting"
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
