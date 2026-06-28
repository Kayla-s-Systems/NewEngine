use super::*;

#[inline]
pub(super) fn parse_plugin_draw_list_provider(
    plugin_id: &str,
    describe_json: &str,
) -> Option<ExternalRenderDrawListProviderDesc> {
    let parsed: PluginDrawListProviderJson = serde_json::from_str(describe_json).ok()?;
    let id = parsed
        .id
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| format!("{plugin_id}.render_draw_lists"));
    let label = parsed
        .label
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let mut tags = parsed.tags.unwrap_or_default();
    push_unique_string(&mut tags, PROVIDER_TAG_PLUGIN);
    let mut capabilities = parsed.capabilities.unwrap_or_default();
    push_unique_string(&mut capabilities, PROVIDER_CAP_DRAW_LISTS);
    let mut draw_lists = Vec::new();
    for item in parsed.draw_lists.unwrap_or_default() {
        if let Some(kind) = parse_draw_list_kind(&item) {
            if !draw_lists.contains(&kind) {
                draw_lists.push(kind);
            }
        } else {
            newengine_ulog_api::ulog::warn!(
                "render draw-list provider registry: plugin='{}' provider='{}' declares unknown draw_list='{}'",
                plugin_id,
                id,
                item
            );
        }
    }
    let gateway_id = parsed
        .engine_gateway
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| newengine_core::render::ENGINE_RENDER_DRAW_LISTS_SERVICE_ID.to_owned());
    if !newengine_service_api::engine_gateway_matches_service_kind(
        &gateway_id,
        newengine_core::render::RENDER_DRAW_LIST_PROVIDER_SERVICE_KIND,
    ) {
        newengine_ulog_api::ulog::warn!(
            "render draw-list provider registry: plugin='{}' provider='{}' declares gateway='{}' but expected service_kind='{}'",
            plugin_id,
            id,
            gateway_id,
            newengine_core::render::RENDER_DRAW_LIST_PROVIDER_SERVICE_KIND
        );
        return None;
    }
    let method = parsed
        .method
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| {
            newengine_core::render::RENDER_DRAW_LIST_PROVIDER_METHOD_EXTRACT.to_string()
        });

    Some(ExternalRenderDrawListProviderDesc {
        id,
        plugin_id: plugin_id.to_string(),
        label,
        tags,
        capabilities,
        draw_lists,
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

#[inline]
fn parse_draw_list_kind(value: &str) -> Option<RenderDrawListKind> {
    match value.trim() {
        "shadow_casters" | "ShadowCasters" | "shadow" => Some(RenderDrawListKind::ShadowCasters),
        "opaque_forward" | "OpaqueForward" | "opaque" => Some(RenderDrawListKind::OpaqueForward),
        "transparent" | "Transparent" => Some(RenderDrawListKind::Transparent),
        "ui" | "Ui" | "UI" => Some(RenderDrawListKind::Ui),
        "debug" | "Debug" => Some(RenderDrawListKind::Debug),
        _ => None,
    }
}

#[inline]
pub(super) fn build_draw_list_provider_request(
    ctx: &SceneExtractionCtx<'_>,
    lists: &RuntimeDrawListSet,
    frame_plan: &newengine_render_frame_graph::RenderFramePlan,
) -> DrawListProviderExtractRequest {
    let routes = FrameGraphRoutes {
        routes: frame_plan
            .graph
            .passes
            .iter()
            .map(|pass| FrameGraphRoute {
                pass: pass.kind,
                draw_lists: pass.draw_lists.clone(),
            })
            .collect(),
    };

    DrawListProviderExtractRequest {
        scene: SceneExtractionSnapshot {
            frame_index: frame_plan.graph.frame_index,
            viewport_extent: ctx.viewport_extent,
            surface_extent: ctx.surface_extent,
            runtime: ctx.runtime,
            debug_overlays: ctx.debug_overlays,
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
                position_ws: [
                    ctx.camera_position.x,
                    ctx.camera_position.y,
                    ctx.camera_position.z,
                ],
            },
            active_draw_lists: lists.kinds().into_iter().collect(),
        },
        visibility: VisibilityMask {
            shadow_casters: ctx.visibility().shadow_casters,
            opaque_forward: ctx.visibility().opaque_forward,
            transparent: ctx.visibility().transparent,
            ui: ctx.visibility().ui,
            debug: ctx.visibility().debug,
        },
        routes,
    }
}
