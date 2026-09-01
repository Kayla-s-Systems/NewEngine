#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::{
    RenderApi, RenderDrawListKind, RenderFrameEnvelope, RenderGraphPassKind,
    RenderGraphSubmitReport,
};
use newengine_core::EngineResult;

#[inline]
pub(super) fn submit_frame_envelope(
    r: &mut dyn RenderApi,
    frame: RenderFrameEnvelope,
    trace_frame: bool,
) -> EngineResult<RenderGraphSubmitReport> {
    let graph_label = trace_frame.then(|| {
        frame
            .label
            .clone()
            .unwrap_or_else(|| "<unnamed>".to_owned())
    });
    let report = r.submit_frame(frame)?;
    if trace_frame {
        newengine_ulog_api::ulog::debug!(
            "render frame envelope: submitted graph='{}' passes={} executed_native={} skipped_native={} barriers(raw={}, war={}, waw={})",
            graph_label.as_deref().unwrap_or("<unnamed>"),
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
            newengine_ulog_api::ulog::debug!(
                "render frame envelope: draw-list replay stats [{}]",
                draw_lists
            );
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

#[inline]
pub fn record_render_phase<T>(
    r: &mut dyn RenderApi,
    phase: RenderGraphPassKind,
    record: impl FnOnce(&mut dyn RenderApi) -> EngineResult<T>,
) -> EngineResult<T> {
    r.begin_render_phase(phase)?;
    let record_result = record(r);
    let end_result = r.end_render_phase();
    match (record_result, end_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
    }
}
