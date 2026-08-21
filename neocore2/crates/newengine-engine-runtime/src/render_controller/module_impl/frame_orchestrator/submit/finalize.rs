use super::shadow_debug::shadow_torture_acceptance_trace_enabled;
use super::*;

pub(super) struct SuccessfulSubmit<'a> {
    pub scope: RenderFrameScope,
    pub view_frame: &'a crate::scene_bridge::EngineViewGatewayFrame,
    pub base_lights: newengine_render_feature_api::PackedLights,
    pub render_shadow_map: bool,
    pub shadow_plan: shadows::LightShadowPlan,
    pub render_local_shadow_map: bool,
    pub local_shadow_plan: shadows::LocalShadowPlan,
    pub frame_plan: &'a newengine_render_frame_graph::RenderFramePlan,
    pub submit_report: newengine_core::render::RenderGraphSubmitReport,
}

pub(super) fn finalize_successful_submit(
    controller: &mut RuntimeRenderController,
    submitted: SuccessfulSubmit<'_>,
) -> PlayableFrameOutcome {
    let SuccessfulSubmit {
        scope,
        view_frame,
        base_lights,
        render_shadow_map,
        shadow_plan,
        render_local_shadow_map,
        local_shadow_plan,
        frame_plan,
        submit_report,
    } = submitted;

    if render_shadow_map {
        controller.mark_shadow_map_rendered(shadow_plan);
    }
    if render_local_shadow_map {
        controller.mark_local_shadow_map_rendered(local_shadow_plan);
    }

    let view = view_frame.view;
    if shadow_torture_acceptance_trace_enabled() && controller.frame.frame_index.is_multiple_of(120)
    {
        newengine_ulog_api::ulog::info!(
            "shadow torture acceptance: frame={} pass(directional={} local={}) cache(directional_valid={} directional_reuse={} directional_refresh[cold={} projection={} mismatch[texture={} matrix={} split={} params={} extra={}] caster={}] local_valid={} local_reuse={} local_refresh={}) caster_revision={} caster_changes[entity={} bounds={} geometry={} material={} visibility={}] camera=({:.5},{:.5},{:.5}) forward=({:.6},{:.6},{:.6}) light_dir=({:.6},{:.6},{:.6}) jitter=({:.5},{:.5})",
            controller.frame.frame_index,
            render_shadow_map,
            render_local_shadow_map,
            controller.shadows.cache_valid,
            controller.shadows.cache_reuse_count,
            controller.shadows.cache_cold_refresh_count,
            controller.shadows.cache_projection_refresh_count,
            controller.shadows.cache_projection_texture_refresh_count,
            controller.shadows.cache_projection_matrix_refresh_count,
            controller.shadows.cache_projection_split_refresh_count,
            controller.shadows.cache_projection_params_refresh_count,
            controller.shadows.cache_projection_extra_refresh_count,
            controller.shadows.cache_caster_refresh_count,
            controller.shadows.local_cache_valid,
            controller.shadows.local_cache_reuse_count,
            controller.shadows.local_cache_refresh_count,
            controller.shadows.caster_revision,
            controller.shadows.caster_entity_change_count,
            controller.shadows.caster_bounds_change_count,
            controller.shadows.caster_geometry_change_count,
            controller.shadows.caster_material_change_count,
            controller.shadows.caster_visibility_change_count,
            view.position_ws.x,
            view.position_ws.y,
            view.position_ws.z,
            view.forward_ws.x,
            view.forward_ws.y,
            view.forward_ws.z,
            base_lights.dir_dir_intensity[0],
            base_lights.dir_dir_intensity[1],
            base_lights.dir_dir_intensity[2],
            view_frame.camera_snapshot.jitter_px[0],
            view_frame.camera_snapshot.jitter_px[1],
        );
    }

    controller
        .diagnostics
        .overlay_metrics
        .record_graph_submit(submit_report.clone());

    let mut debug_notes = Vec::new();
    if let Some(report) = view_frame.diagnostics.clone() {
        controller
            .diagnostics
            .overlay_metrics
            .record_view_report(report.clone());
        debug_notes.push(format!(
            "view director={} mode={} view={} dominant={:?} rendered={} input={} lock={} gate_blocked={} blend_active={} blend_alpha={:.3} events={}",
            report.active_director,
            report.active_mode,
            report.active_view_mode,
            report.dominant_director,
            report.rendered_director_count,
            report.input_context,
            report.director_lock_input,
            report.gate_blocked,
            report.frame_blend_active,
            report.frame_blend_alpha,
            report.pending_event_count,
        ));
        if report.transition.phase != EngineViewTransitionPhase::Idle {
            debug_notes.push(format!(
                "view transition {:?} {:.2}s target={:?}",
                report.transition.phase, report.transition.elapsed_sec, report.target_entity,
            ));
        }
    }

    PlayableFrameOutcome::Continue {
        frame_debug_snapshot: Some(RenderFrameDebugSnapshot {
            frame_index: controller.frame.frame_index,
            surface_extent: [scope.w, scope.h],
            viewport_extent: [scope.vp_w, scope.vp_h],
            direct_surface_viewport: scope.direct_surface_viewport,
            graph_label: frame_plan
                .graph
                .label
                .clone()
                .unwrap_or_else(|| "<unnamed>".to_owned()),
            phase_order: frame_plan
                .phase_order()
                .map(|phase| phase.label().to_owned())
                .collect(),
            draw_list_stats: submit_report.draw_list_stats.clone(),
            executed_passes: submit_report.executed_passes,
            skipped_passes: submit_report.skipped_passes,
            cpu_record_ms: submit_report.cpu_record_ms,
            gpu_submit_ms: submit_report.gpu_submit_ms,
            queued_upload_jobs: 0,
            queued_upload_bytes: 0,
            resource_buffers: 0,
            resource_textures: 0,
            resource_pipelines: 0,
            notes: debug_notes,
        }),
    }
}
