use super::*;
use crate::{standard_runtime_frame, StandardRuntimePipelineDesc};
use newengine_render_api::{Extent2D, RenderGraphPassKind, RenderGraphResourceUsage, RenderTargetId};

#[test]
fn deferred_runtime_graph_uses_explicit_lighting_resolve_without_forward_replay() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            7,
            Extent2D::new(1280, 720),
            Extent2D::new(1280, 720),
        )
        .deferred(true)
        .postfx(false)
        .ui(false)
        .debug_overlay(false),
    );
    let graph = plan.graph;
    let gbuffer_index = graph
        .passes
        .iter()
        .position(|pass| pass.kind == RenderGraphPassKind::GBuffer)
        .expect("deferred graph must contain a GBuffer pass");
    let lighting_index = graph
        .passes
        .iter()
        .position(|pass| pass.kind == RenderGraphPassKind::DeferredLighting)
        .expect("deferred graph must contain a DeferredLighting resolve pass");
    assert!(
        gbuffer_index < lighting_index,
        "DeferredLighting must execute after GBuffer"
    );

    let lighting = &graph.passes[lighting_index];
    assert!(
        lighting.draw_lists.is_empty(),
        "DeferredLighting is a fullscreen resolve pass and must not replay OpaqueForward draw lists"
    );
    for resource in [
        RG_GBUFFER_ALBEDO,
        RG_GBUFFER_NORMAL,
        RG_GBUFFER_MATERIAL,
        RG_GBUFFER_DEPTH,
    ] {
        assert!(
            lighting
                .reads
                .iter()
                .any(|read| read.resource == resource && read.usage == RenderGraphResourceUsage::SampledTexture),
            "DeferredLighting must sample GBuffer resource {:?}",
            resource
        );
    }
    assert!(
        lighting
            .writes
            .iter()
            .any(|write| write.resource == RG_LIT_COLOR && write.usage == RenderGraphResourceUsage::ColorAttachment),
        "DeferredLighting must write lit color"
    );
}

#[test]
fn forward_runtime_graph_does_not_emit_deferred_resolve() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            8,
            Extent2D::new(640, 360),
            Extent2D::new(640, 360),
        )
        .deferred(false)
        .postfx(false)
        .ui(false)
        .debug_overlay(false),
    );
    assert!(
        !plan
            .graph
            .passes
            .iter()
            .any(|pass| pass.kind == RenderGraphPassKind::DeferredLighting),
        "forward graph must not emit deferred lighting resolve"
    );
}


#[test]
fn hdr_scene_with_postfx_writes_surface_through_postfx() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            9,
            Extent2D::new(1280, 720),
            Extent2D::new(1280, 720),
        )
        .hdr_scene(true)
        .postfx(true)
        .ui(false)
        .debug_overlay(false),
    );

    let postfx = plan
        .graph
        .passes
        .iter()
        .find(|pass| pass.kind == RenderGraphPassKind::PostFx)
        .expect("HDR scene with postFX must emit PostFx surface resolve");
    assert!(postfx
        .reads
        .iter()
        .any(|read| read.resource == RG_SCENE_HDR_COLOR && read.usage == RenderGraphResourceUsage::SampledTexture));
    assert!(postfx
        .writes
        .iter()
        .any(|write| write.resource == RG_SURFACE_COLOR && write.usage == RenderGraphResourceUsage::ColorAttachment));
    assert!(!plan
        .graph
        .passes
        .iter()
        .any(|pass| pass.kind == RenderGraphPassKind::Copy && pass.label == "hdr_scene_resolve_to_surface"));
}

#[test]
fn hdr_scene_without_postfx_adds_surface_resolve() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            10,
            Extent2D::new(1280, 720),
            Extent2D::new(1280, 720),
        )
        .hdr_scene(true)
        .postfx(false)
        .ui(false)
        .debug_overlay(false),
    );

    let resolve = plan
        .graph
        .passes
        .iter()
        .find(|pass| pass.kind == RenderGraphPassKind::Copy && pass.label == "hdr_scene_resolve_to_surface")
        .expect("HDR scene without postFX must still resolve scene color into the surface");
    assert!(resolve
        .reads
        .iter()
        .any(|read| read.resource == RG_SCENE_HDR_COLOR && read.usage == RenderGraphResourceUsage::SampledTexture));
    assert!(resolve
        .writes
        .iter()
        .any(|write| write.resource == RG_SURFACE_COLOR && write.usage == RenderGraphResourceUsage::ColorAttachment));
}

#[test]
fn ui_only_composite_writes_surface_without_scene_resolve() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            11,
            Extent2D::new(800, 600),
            Extent2D::new(800, 600),
        )
        .hdr_scene(false)
        .postfx(false)
        .ui(true)
        .debug_overlay(false),
    );

    assert!(plan
        .graph
        .passes
        .iter()
        .any(|pass| pass.kind == RenderGraphPassKind::UiComposite
            && pass.writes.iter().any(|write| write.resource == RG_SURFACE_COLOR)));
    assert!(!plan
        .graph
        .passes
        .iter()
        .any(|pass| pass.kind == RenderGraphPassKind::Copy && pass.label == "hdr_scene_resolve_to_surface"));
}

#[test]
fn debug_overlay_runs_after_ui_composite() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            12,
            Extent2D::new(800, 600),
            Extent2D::new(800, 600),
        )
        .postfx(false)
        .ui(true)
        .debug_overlay(true),
    );
    let ui = plan
        .graph
        .passes
        .iter()
        .position(|pass| pass.kind == RenderGraphPassKind::UiComposite)
        .expect("UI composite pass must exist");
    let debug = plan
        .graph
        .passes
        .iter()
        .position(|pass| pass.kind == RenderGraphPassKind::DebugOverlay)
        .expect("Debug overlay pass must exist");
    assert!(ui < debug, "Debug overlay must execute after UI composite");
}

#[test]
fn shadow_cascade_atlas_scales_with_cascade_count() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            13,
            Extent2D::new(800, 600),
            Extent2D::new(800, 600),
        )
        .shadow(true, 1024)
        .shadow_cascades(5)
        .postfx(false)
        .ui(false)
        .debug_overlay(false),
    );
    let shadow = plan
        .graph
        .resources
        .iter()
        .find(|resource| resource.id == RG_SHADOW_MAP)
        .expect("cascade shadow map resource must exist");
    assert_eq!(shadow.extent, Some(Extent2D::new(4096, 2048)));
}

#[test]
fn draw_list_route_validation_warns_when_pass_route_has_no_declared_list() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            14,
            Extent2D::new(800, 600),
            Extent2D::new(800, 600),
        )
        .hdr_scene(false)
        .postfx(false)
        .ui(true)
        .debug_overlay(false),
    );

    let report = plan.validate_draw_list_routes();
    assert!(report.ok, "missing declared route is a warning, not a hard graph error");
    assert!(report
        .warnings
        .iter()
        .any(|issue| issue.code == "draw_list.route_without_declared_list"));
}

#[test]
fn external_viewport_target_is_declared_when_requested() {
    let target = RenderTargetId(std::num::NonZeroU32::new(77).unwrap());
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            15,
            Extent2D::new(1280, 720),
            Extent2D::new(640, 360),
        )
        .hdr_scene(false)
        .viewport_render_target(Some(target))
        .postfx(false)
        .ui(false)
        .debug_overlay(false),
    );

    let viewport = plan
        .graph
        .resources
        .iter()
        .find(|resource| resource.id == RG_VIEWPORT_COLOR)
        .expect("external viewport color resource must exist");
    assert_eq!(viewport.external, Some(newengine_render_api::RenderGraphExternalResource::RenderTarget(target)));
}

#[test]
fn viewport_surface_with_hdr_scene_uses_offscreen_scene_color_and_depth() {
    let plan = standard_runtime_frame(
        StandardRuntimePipelineDesc::new(
            16,
            Extent2D::new(1280, 720),
            Extent2D::new(1280, 720),
        )
        .viewport_is_surface(true)
        .hdr_scene(true)
        .postfx(false)
        .ui(false)
        .debug_overlay(false),
    );

    assert!(plan
        .graph
        .resources
        .iter()
        .any(|resource| resource.id == RG_SCENE_HDR_COLOR && resource.external.is_none()));
    let depth = plan
        .graph
        .resources
        .iter()
        .find(|resource| resource.id == RG_VIEWPORT_DEPTH)
        .expect("HDR scene must use offscreen scene depth");
    assert!(depth.external.is_none());
    assert!(plan
        .graph
        .passes
        .iter()
        .any(|pass| pass.kind == RenderGraphPassKind::Copy && pass.label == "hdr_scene_resolve_to_surface"));
}
