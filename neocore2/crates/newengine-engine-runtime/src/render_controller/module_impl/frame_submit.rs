#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{RenderApi, RenderDrawListKind, RenderGraphSubmitReport};
use newengine_core::EngineResult;
use newengine_render_frame_graph::RenderFramePlan;

#[inline]
pub(super) fn submit_frame_plan_v3(
    r: &mut dyn RenderApi,
    plan: &RenderFramePlan,
    trace_frame: bool,
) -> EngineResult<RenderGraphSubmitReport> {
    let report = r.submit_render_graph(plan.graph.clone())?;
    if trace_frame {
        log::debug!(
            "render frame graph v3: submitted graph='{}' passes={} executed_native={} skipped_native={} barriers(raw={}, war={}, waw={})",
            plan.graph.label.as_deref().unwrap_or("<unnamed>"),
            report.compile.pass_count,
            report.executed_passes,
            report.skipped_passes,
            report.compile.barriers.read_after_write,
            report.compile.barriers.write_after_read,
            report.compile.barriers.write_after_write,
        );
        if !report.draw_list_stats.is_empty() {
            let draw_lists = report
                .draw_list_stats
                .iter()
                .map(|it| {
                    format!(
                        "{}: recorded={} draw={} indexed={} skipped={} state(vp={},sc={},pipe={},vb={},ib={},bg={},invalid={})",
                        it.draw_list.label(),
                        it.recorded_commands,
                        it.draw_calls,
                        it.indexed_draw_calls,
                        it.skipped_commands,
                        it.viewport_sets,
                        it.scissor_sets,
                        it.pipeline_binds,
                        it.vertex_buffer_binds,
                        it.index_buffer_binds,
                        it.bind_group_binds,
                        it.invalid_draw_calls,
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            log::debug!("render frame graph v3: draw-list replay stats [{}]", draw_lists);
        }
    }
    Ok(report)
}

#[inline]
pub fn record_draw_list<T>(
    r: &mut dyn RenderApi,
    kind: RenderDrawListKind,
    record: impl FnOnce(&mut dyn RenderApi) -> EngineResult<T>,
) -> EngineResult<T> {
    r.begin_draw_list(kind)?;
    let record_result = record(r);
    let end_result = r.end_draw_list();
    match (record_result, end_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}
