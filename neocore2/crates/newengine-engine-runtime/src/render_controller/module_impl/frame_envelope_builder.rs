#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{Extent2D, PostFxFrameParams, RenderEffectStack, RenderFrameDomainIntent, RenderFrameEnvelope};
use newengine_render_frame_graph::{DrawListDesc, RenderFramePlan};

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
    draw_list_descs: &[DrawListDesc],
    postfx: PostFxFrameParams,
    trace_frame: bool,
) -> RenderFrameEnvelope {
    if trace_frame {
        let phases = frame_plan
            .phase_order()
            .map(|phase| phase.label())
            .collect::<Vec<_>>()
            .join(" -> ");
        log::debug!("render frame envelope: frame={} phases={}", frame_index, phases);
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
    .with_draw_lists(draw_list_descs.iter().map(|desc| desc.kind))
}
