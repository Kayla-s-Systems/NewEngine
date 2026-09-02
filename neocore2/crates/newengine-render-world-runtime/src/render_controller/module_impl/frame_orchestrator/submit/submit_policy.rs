use super::*;

pub(super) enum PreparedFrameSubmit {
    Submitted(newengine_core::render::RenderGraphSubmitReport),
    EndedEarly,
    BackendDeferred,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn submit_prepared_frame(
    controller: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    frame_envelope: newengine_core::render::RenderFrameEnvelope,
    frame_plan: &newengine_render_frame_graph::RenderFramePlan,
    draw_list_descs: &[newengine_render_frame_graph::DrawListDesc],
    scope: RenderFrameScope,
    rt: Option<RenderTargetId>,
    shadow_rt_for_graph: Option<RenderTargetId>,
    local_shadow_rt: Option<RenderTargetId>,
) -> EngineResult<PreparedFrameSubmit> {
    let submit_report = match submit_frame_envelope(r, frame_envelope, scope.trace_frame) {
        Ok(report) => report,
        Err(e) if is_transient_shader_pipeline_error(&e) => {
            // Graph execution may already have recorded native Vulkan commands.
            // Never present this partial frame: abort the opened backend frame and
            // retry from a fresh command buffer once the async shader becomes ready.
            RenderFrameOrchestrator::abort_viewport_after_transient_pipeline_wait(
                controller, r, scope, e,
            )?;
            return Ok(PreparedFrameSubmit::EndedEarly);
        }
        Err(e) => {
            let message = e.to_string();
            controller.disable_viewport_pass("render_graph.submit_frame", &message);
            let pass_detail = frame_plan
            .graph
            .passes
            .iter()
            .map(|pass| {
                format!(
                    "id={} label='{}' kind={:?} domain={:?} queue={:?} reads={:?} writes={:?} creates={:?} draw_lists={:?}",
                    pass.id.0,
                    pass.label,
                    pass.kind,
                    pass.domain,
                    pass.queue,
                    pass.reads,
                    pass.writes,
                    pass.creates,
                    pass.draw_lists,
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
            let resource_detail = frame_plan
            .graph
            .resources
            .iter()
            .map(|resource| {
                format!(
                    "id={} label={:?} semantic={:?} usage={:?} lifetime={:?} extent={:?} format={:?} samples={} external={:?}",
                    resource.id.0,
                    resource.label,
                    resource.semantic,
                    resource.usage,
                    resource.lifetime,
                    resource.extent,
                    resource.format,
                    resource.sample_count,
                    resource.external,
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
            let expected_draw_lists = draw_list_descs
                .iter()
                .map(|desc| {
                    format!(
                        "{}:draw={} indexed={} triangles={} instances={}",
                        desc.kind.label(),
                        desc.stats.draw_calls,
                        desc.stats.indexed_draw_calls,
                        desc.stats.triangle_count,
                        desc.stats.instance_count,
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            newengine_ulog_api::ulog::error!(
            "CRITICAL render regression: viewport scene pass disabled frame={} viewport={}x{} surface={}x{} direct_surface={} viewport_rt={:?} shadow_rt={:?} local_shadow_rt={:?} graph_passes={} graph_resources={} expected_draw_lists='{}' fallback='degraded-ui-safe-present' reason='{}'",
            controller.frame.frame_index,
            scope.vp_w,
            scope.vp_h,
            scope.w,
            scope.h,
            scope.direct_surface_viewport,
            rt,
            shadow_rt_for_graph,
            local_shadow_rt,
            frame_plan.graph.passes.len(),
            frame_plan.graph.resources.len(),
            expected_draw_lists,
            message,
        );
            newengine_ulog_api::ulog::error!(
                "CRITICAL render regression graph passes: {}",
                pass_detail
            );
            newengine_ulog_api::ulog::error!(
                "CRITICAL render regression graph resources: {}",
                resource_detail
            );
            newengine_ulog_api::ulog::error!(
            "render controller: frame graph submit failed; viewport pass disabled and renderer continues in degraded UI/safe-present mode: {}",
            message
        );
            // Any error returned after submit_frame started consuming the graph
            // may leave native commands in the backend command buffer. Abort rather
            // than attempting to present a partially recorded frame.
            let abort_result = r.abort_frame();
            if is_backend_device_lost_error(&e) {
                if let Err(abort_error) = abort_result {
                    newengine_ulog_api::ulog::warn!(
                        "render controller: abort after device loss also failed: {}",
                        abort_error
                    );
                }
                controller.record_render_backend_error("render_graph.submit_frame", e)?;
            } else {
                abort_result?;
            }
            return Ok(PreparedFrameSubmit::EndedEarly);
        }
    };

    if submit_report.backend_deferred {
        if scope.trace_frame
            || controller.frame.frame_index <= 3
            || controller.frame.frame_index.is_multiple_of(120)
        {
            newengine_ulog_api::ulog::debug!(
            "render controller: backend deferred frame={} graph_passes={} skipped_passes={} viewport={}x{} direct_surface={} policy='bounded backend/WSI back-pressure; return to host event loop and retry next redraw'",
            controller.frame.frame_index,
            frame_plan.graph.passes.len(),
            submit_report.skipped_passes,
            scope.vp_w,
            scope.vp_h,
            scope.direct_surface_viewport,
        );
        }
        RenderFrameOrchestrator::publish_render_task_pass_event(
        controller.frame.frame_index,
        newengine_task_api::task_pass::RENDER_SUBMIT,
        newengine_task_api::EngineTaskPhase::Completed,
        "Render submit deferred by backend",
        "Backend did not acquire/open a native frame within its bounded scheduling window; no graph failure occurred and the next host redraw will retry.",
        None,
    );
        return Ok(PreparedFrameSubmit::BackendDeferred);
    }

    let expected_opaque_draws = draw_list_descs
        .iter()
        .find(|desc| desc.kind == newengine_core::render::RenderDrawListKind::OpaqueForward)
        .map(|desc| {
            desc.stats
                .draw_calls
                .saturating_add(desc.stats.indexed_draw_calls)
        })
        .unwrap_or(0);
    if expected_opaque_draws > 0 {
        let opaque_stats = submit_report.draw_list_stats.iter().find(|stats| {
            stats.draw_list == newengine_core::render::RenderDrawListKind::OpaqueForward
        });
        let recorded_opaque_draws = opaque_stats
            .map(|stats| stats.draw_calls.saturating_add(stats.indexed_draw_calls))
            .unwrap_or(0);
        if recorded_opaque_draws == 0 {
            let skipped = opaque_stats
                .map(|stats| stats.skipped_commands)
                .unwrap_or(0);
            let invalid = opaque_stats
                .map(|stats| stats.invalid_draw_calls)
                .unwrap_or(0);
            newengine_ulog_api::ulog::error!(
            "CRITICAL render regression: scene-present invariant violated frame={} expected_opaque_draws={} recorded_opaque_draws=0 skipped_commands={} invalid_draw_calls={} executed_passes={} skipped_passes={} viewport={}x{} direct_surface={} viewport_rt={:?}",
            controller.frame.frame_index,
            expected_opaque_draws,
            skipped,
            invalid,
            submit_report.executed_passes,
            submit_report.skipped_passes,
            scope.vp_w,
            scope.vp_h,
            scope.direct_surface_viewport,
            rt,
        );
        }
    }
    if unexpected_zero_pass_submit(frame_plan.graph.passes.len(), &submit_report) {
        newengine_ulog_api::ulog::error!(
        "CRITICAL render regression: non-empty frame graph executed zero passes frame={} declared_passes={} declared_resources={} skipped_passes={} viewport={}x{} direct_surface={}",
        controller.frame.frame_index,
        frame_plan.graph.passes.len(),
        frame_plan.graph.resources.len(),
        submit_report.skipped_passes,
        scope.vp_w,
        scope.vp_h,
        scope.direct_surface_viewport,
    );
    }

    Ok(PreparedFrameSubmit::Submitted(submit_report))
}
