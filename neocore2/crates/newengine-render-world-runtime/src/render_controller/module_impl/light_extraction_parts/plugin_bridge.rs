use super::*;

pub(super) fn build_light_provider_request(
    ctx: &LightExtractionCtx<'_>,
) -> LightExtractionProviderRequest {
    LightExtractionProviderRequest {
        scene: LightExtractionSnapshot {
            frame_index: ctx.frame_index,
            viewport_extent: ctx.viewport_extent,
            surface_extent: ctx.surface_extent,
            bounds: RenderBoundsSnapshot {
                center: [
                    ctx.bounds.center.x,
                    ctx.bounds.center.y,
                    ctx.bounds.center.z,
                ],
                radius: ctx.bounds.radius,
            },
            view: RenderViewSnapshot {
                view_projection_cols: ctx.viewproj.to_cols_array_2d(),
                position_ws: ctx.camera_position,
            },
        },
        settings: ShadowSettingsSnapshot {
            enabled: ctx.settings.enabled,
            method: shadow_method_label(ctx.settings.method).to_string(),
            filter: shadow_filter_label(ctx.settings.filter).to_string(),
            resolution: ctx.settings.resolution,
            max_distance: ctx.settings.max_distance,
            bias: ctx.settings.bias,
            softness: ctx.settings.softness,
            contact_strength: ctx.settings.contact_strength,
            normal_bias: ctx.settings.normal_bias,
            cascade_count: ctx.settings.cascade_count,
            pcss: newengine_render_api::ShadowPcssSettingsSnapshot {
                light_angular_radius_degrees: ctx.settings.pcss.light_angular_radius_degrees,
                blocker_search_radius_texels: ctx.settings.pcss.blocker_search_radius_texels,
                max_filter_radius_texels: ctx.settings.pcss.max_filter_radius_texels,
                blocker_samples: ctx.settings.pcss.blocker_samples,
                filter_samples: ctx.settings.pcss.filter_samples,
                min_filter_radius_texels: ctx.settings.pcss.min_filter_radius_texels,
                stable_kernel_cell_texels: ctx.settings.pcss.stable_kernel_cell_texels,
            },
        },
        backend: BackendShadowCapabilities {
            directional_depth_map: true,
            cascaded_shadow_maps: true,
            point_cube_map: false,
            spot_depth_map: false,
            max_shadow_resolution: ctx.settings.resolution,
            max_directional_cascades: ctx.settings.cascade_count.clamp(1, 4),
            shadow_atlas: true,
        },
    }
}

#[inline]
pub(super) fn light_plan_from_contribution(
    ctx: &LightExtractionCtx<'_>,
    contribution: LightPlanContribution,
) -> LightShadowPlan {
    let resolution = contribution.resolution.max(1);
    let fallback = ctx.lit.white_texture;
    let kind = match contribution.kind {
        LightPlanContributionKind::Directional => {
            super::super::super::shadows::ShadowLightKind::Directional
        }
        LightPlanContributionKind::Point => super::super::super::shadows::ShadowLightKind::Point,
        LightPlanContributionKind::Spot => super::super::super::shadows::ShadowLightKind::Spot,
        LightPlanContributionKind::AmbientOcclusion | LightPlanContributionKind::None => {
            return LightShadowPlan::disabled(fallback);
        }
    };

    if !contribution.supported {
        return LightShadowPlan::unsupported(kind, fallback, resolution);
    }

    let (Some(rt), Some(tex)) = (contribution.render_target, contribution.shadow_texture) else {
        return LightShadowPlan::unsupported(kind, fallback, resolution);
    };

    let rt = RenderTargetId::new(rt);
    let tex = TextureId::new(tex);
    let mvp = Mat4::from_cols_array_2d(&contribution.light_mvp_cols);

    match kind {
        super::super::super::shadows::ShadowLightKind::Directional => LightShadowPlan::directional(
            rt,
            tex,
            resolution,
            mvp,
            contribution.params,
            contribution.extra,
            None,
        )
        .with_pcss(contribution.pcss0, contribution.pcss1),
        super::super::super::shadows::ShadowLightKind::Point
        | super::super::super::shadows::ShadowLightKind::Spot => {
            LightShadowPlan::unsupported(kind, fallback, resolution)
        }
    }
}

#[inline]
const fn shadow_filter_label(filter: newengine_lighting::ShadowFilter) -> &'static str {
    match filter {
        newengine_lighting::ShadowFilter::Hard => "hard",
        newengine_lighting::ShadowFilter::Pcf => "pcf",
        newengine_lighting::ShadowFilter::Pcss => "pcss",
    }
}

#[inline]
const fn shadow_method_label(method: ShadowMethod) -> &'static str {
    match method {
        ShadowMethod::None => "none",
        ShadowMethod::DirectionalDepthMap => "directional_depth_map",
        ShadowMethod::CascadedShadowMaps => "cascaded_shadow_maps",
        ShadowMethod::PointCubeMap => "point_cube_map",
        ShadowMethod::SpotDepthMap => "spot_depth_map",
    }
}

#[inline]
pub(super) fn parse_plugin_light_provider(
    plugin_id: &str,
    describe_json: &str,
) -> Option<ExternalLightExtractionProviderDesc> {
    let parsed: PluginLightProviderJson = serde_json::from_str(describe_json).ok()?;
    let id = parsed
        .id
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| format!("{plugin_id}.light_extraction"));
    let label = parsed
        .label
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let mut tags = parsed.tags.unwrap_or_default();
    push_unique_string(&mut tags, LIGHT_PROVIDER_TAG_PLUGIN);
    let mut capabilities = parsed.capabilities.unwrap_or_default();
    push_unique_string(&mut capabilities, LIGHT_PROVIDER_CAP_EXTRACTION);
    let gateway_id = parsed
        .engine_gateway
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| {
            newengine_core::render::ENGINE_RENDER_LIGHT_EXTRACTION_SERVICE_ID.to_owned()
        });
    if !newengine_service_api::engine_gateway_matches_service_kind(
        &gateway_id,
        newengine_core::render::RENDER_LIGHT_EXTRACTION_PROVIDER_SERVICE_KIND,
    ) {
        newengine_ulog_api::ulog::warn!(
            "render light extraction registry: plugin='{}' provider='{}' declares gateway='{}' but expected service_kind='{}'",
            plugin_id,
            id,
            gateway_id,
            newengine_core::render::RENDER_LIGHT_EXTRACTION_PROVIDER_SERVICE_KIND
        );
        return None;
    }
    let method = parsed
        .method
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| {
            newengine_core::render::RENDER_LIGHT_EXTRACTION_PROVIDER_METHOD_EXTRACT.to_string()
        });

    Some(ExternalLightExtractionProviderDesc {
        id,
        plugin_id: plugin_id.to_string(),
        label,
        tags,
        capabilities,
        light_kinds: parsed.light_kinds.unwrap_or_default(),
        gateway_id,
        method,
    })
}

#[inline]
fn push_unique_string(dst: &mut Vec<String>, value: &str) {
    if !dst.iter().any(|it| it == value) {
        dst.push(value.to_string());
    }
}
