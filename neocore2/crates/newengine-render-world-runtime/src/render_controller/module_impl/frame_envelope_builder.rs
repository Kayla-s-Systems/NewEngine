#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    Extent2D, PostFxFrameParams, RenderEffectStack, RenderFrameDomainIntent, RenderFrameEnvelope,
    UiLayerDrawPacketSet,
};
use newengine_render_frame_graph::{ui_layer_only_frame, RenderFramePlan};

/// Builds the backend-facing envelope from an engine-side frame plan.
///
/// The orchestrator owns scene extraction and provider execution. This builder is
/// the final typed boundary before the backend adapter receives work.
pub(super) fn build_runtime_frame_envelope(
    frame_index: u64,
    clear_color: [f32; 4],
    surface_extent: Extent2D,
    viewport_extent: Extent2D,
    direct_surface_viewport: bool,
    frame_plan: &RenderFramePlan,
    postfx: PostFxFrameParams,
    ui_layers: UiLayerDrawPacketSet,
    trace_frame: bool,
) -> RenderFrameEnvelope {
    if trace_frame {
        let phases = frame_plan
            .phase_order()
            .map(|phase| phase.label())
            .collect::<Vec<_>>()
            .join(" -> ");
        newengine_ulog_api::ulog::debug!(
            "render frame envelope: frame={} phases={}",
            frame_index,
            phases
        );
    }

    let domains = RenderFrameDomainIntent {
        render3d_enabled: true,
        render2d_enabled: true,
        ui_postprocess_enabled: postfx.ui_backdrop.enabled,
    };

    RenderFrameEnvelope::new(
        frame_index,
        clear_color,
        surface_extent,
        viewport_extent,
        direct_surface_viewport,
        frame_plan.graph.clone(),
    )
    .with_postfx(postfx)
    .with_domain_intent(domains)
    .with_effect_stack(RenderEffectStack::aaa_default())
    .with_draw_lists(frame_plan.draw_lists.iter().map(|desc| desc.kind))
    .with_ui_layers(ui_layers)
}

/// Builds a renderer-facing envelope for bootstrap, editor/tool UI-only and degraded frames.
///
/// There is intentionally no `legacy singleton UI draw path` compatibility path
/// here. Even frames without world rendering cross the same typed layer-packet/RenderGraph
/// boundary as normal playable frames.
pub(super) fn build_ui_layer_frame_envelope(
    frame_index: u64,
    clear_color: [f32; 4],
    surface_extent: Extent2D,
    ui_layers: UiLayerDrawPacketSet,
) -> RenderFrameEnvelope {
    let frame_plan = ui_layer_only_frame(
        frame_index,
        surface_extent,
        ui_layers.packets.iter().map(|packet| packet.domain),
    );
    let domains = RenderFrameDomainIntent {
        render3d_enabled: false,
        render2d_enabled: true,
        ui_postprocess_enabled: false,
    };

    RenderFrameEnvelope::new(
        frame_index,
        clear_color,
        surface_extent,
        surface_extent,
        true,
        frame_plan.graph,
    )
    .with_domain_intent(domains)
    .with_effect_stack(RenderEffectStack::default())
    .with_ui_layers(ui_layers)
}
