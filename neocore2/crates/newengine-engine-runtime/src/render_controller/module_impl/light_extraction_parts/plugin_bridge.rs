fn build_light_provider_request(ctx: &LightExtractionCtx<'_>) -> LightExtractionProviderRequest {
    LightExtractionProviderRequest {
        scene: LightExtractionSnapshot {
            frame_index: ctx.frame_index,
            viewport_extent: ctx.viewport_extent,
            surface_extent: ctx.surface_extent,
            bounds: RenderBoundsSnapshot {
                center: [ctx.bounds.center.x, ctx.bounds.center.y, ctx.bounds.center.z],
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
            resolution: ctx.settings.resolution,
            max_distance: ctx.settings.max_distance,
            bias: ctx.settings.bias,
            softness: ctx.settings.softness,
            contact_strength: ctx.settings.contact_strength,
            normal_bias: ctx.settings.normal_bias,
            cascade_count: ctx.settings.cascade_count,
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
fn light_plan_from_contribution(
    ctx: &LightExtractionCtx<'_>,
    contribution: LightPlanContribution,
) -> LightShadowPlan {
    let resolution = contribution.resolution.max(1);
    let fallback = ctx.lit.white_texture;
    let kind = match contribution.kind {
        LightPlanContributionKind::Directional => super::shadows::ShadowLightKind::Directional,
        LightPlanContributionKind::Point => super::shadows::ShadowLightKind::Point,
        LightPlanContributionKind::Spot => super::shadows::ShadowLightKind::Spot,
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
        super::shadows::ShadowLightKind::Directional => {
            LightShadowPlan::directional(rt, tex, resolution, mvp, contribution.params, contribution.extra, None)
        }
        super::shadows::ShadowLightKind::Point | super::shadows::ShadowLightKind::Spot => {
            LightShadowPlan::unsupported(kind, fallback, resolution)
        }
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
fn parse_plugin_light_provider(
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
    let service_id = parsed.service_id.filter(|it| !it.trim().is_empty());
    let method = parsed
        .method
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| newengine_core::render::RENDER_LIGHT_EXTRACTION_PROVIDER_METHOD_EXTRACT.to_string());

    Some(ExternalLightExtractionProviderDesc {
        id,
        plugin_id: plugin_id.to_string(),
        label,
        tags,
        capabilities,
        light_kinds: parsed.light_kinds.unwrap_or_default(),
        service_id,
        method,
    })
}

#[inline]
fn push_unique_string(dst: &mut Vec<String>, value: &str) {
    if !dst.iter().any(|it| it == value) {
        dst.push(value.to_string());
    }
}
