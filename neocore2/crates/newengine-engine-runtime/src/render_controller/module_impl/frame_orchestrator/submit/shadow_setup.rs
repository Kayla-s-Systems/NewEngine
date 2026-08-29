use super::*;

pub(super) struct ShadowSetup {
    pub shadow_plan: shadows::LightShadowPlan,
    pub render_shadow_map: bool,
    pub local_shadow_plan: shadows::LocalShadowPlan,
    pub render_local_shadow_map: bool,
    pub shadow_frame: shadows::ShadowFrame,
    pub local_shadow_frame: shadows::LocalShadowFrame,
    pub world_lights: newengine_render_feature_api::PackedLights,
}

pub(super) fn prepare_shadow_setup(
    controller: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    scene: &Scene,
    plugin_snapshot: Option<&newengine_plugin_host::PluginsSnapshot>,
    snapshot: &SceneRenderSnapshot,
    world_frame: &WorldFrameState,
    lit: newengine_material_domain_api::LitPipeline,
    base_lights: newengine_render_feature_api::PackedLights,
    shadows_enabled: bool,
    extent: Extent2D,
    trace_frame: bool,
) -> ShadowSetup {
    let camera_position = [
        snapshot.camera_position.x,
        snapshot.camera_position.y,
        snapshot.camera_position.z,
    ];
    let shadow_viewproj = world_frame.view_frame.unjittered_view_projection();
    let shadow_plan = if !shadows_enabled {
        shadows::LightShadowPlan::disabled(lit.white_texture)
    } else {
        match shadows::build_light_shadow_plan(
            controller,
            r,
            scene,
            snapshot.bounds,
            lit,
            shadow_viewproj,
            camera_position,
            [
                snapshot.camera_forward.x,
                snapshot.camera_forward.y,
                snapshot.camera_forward.z,
            ],
            extent,
            snapshot.surface_extent,
            plugin_snapshot,
        ) {
            Ok(plan) => plan,
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "render controller: shadow plan disabled for this frame: {}",
                    e
                );
                let _ = r.discard_recorded_commands();
                shadows::LightShadowPlan::disabled(lit.white_texture)
            }
        }
    };

    let render_shadow_map =
        controller.should_render_shadow_map_this_frame(shadow_plan, scene.world());
    controller.set_shadow_caster_cull(if render_shadow_map {
        shadow_plan.caster_cull
    } else {
        None
    });
    RenderFrameOrchestrator::trace_shadow_plan(
        controller,
        trace_frame,
        shadow_plan,
        render_shadow_map,
    );

    let local_shadow_plan = if !shadows_enabled {
        shadows::LocalShadowPlan::disabled(lit.white_texture)
    } else {
        match shadows::build_local_shadow_plan(controller, r, scene.world(), lit, camera_position) {
            Ok(plan) => plan,
            Err(e) => {
                newengine_ulog_api::ulog::warn!(
                    "render controller: local shadow plan disabled for this frame: {}",
                    e
                );
                shadows::LocalShadowPlan::disabled(lit.white_texture)
            }
        }
    };
    let render_local_shadow_map =
        controller.should_render_local_shadow_map_this_frame(local_shadow_plan, scene.world());
    let local_shadow_frame = if local_shadow_plan.is_active()
        && !render_local_shadow_map
        && !controller.shadows.local_cache_valid
    {
        shadows::LocalShadowFrame::disabled(lit.white_texture)
    } else if local_shadow_plan.is_active() && !render_local_shadow_map {
        controller
            .cached_local_shadow_frame()
            .unwrap_or(local_shadow_plan.frame)
    } else {
        local_shadow_plan.frame
    };

    let shadow_frame = if shadow_plan.is_active()
        && !render_shadow_map
        && !controller.shadows.cache_valid
    {
        if trace_frame {
            newengine_ulog_api::ulog::debug!(
                "render shadow cache: using unshadowed fallback until first shadow map is rendered frame={} target={:?}",
                controller.frame.frame_index,
                shadow_plan.render_target()
            );
        }
        shadows::ShadowFrame::disabled(lit.white_texture)
    } else if shadow_plan.is_active() && !render_shadow_map {
        controller
            .cached_shadow_frame()
            .unwrap_or(shadow_plan.frame)
    } else {
        shadow_plan.frame
    };
    let world_lights = base_lights
        .with_shadow_frame(shadow_frame)
        .with_local_shadow_frame(local_shadow_frame);

    ShadowSetup {
        shadow_plan,
        render_shadow_map,
        local_shadow_plan,
        render_local_shadow_map,
        shadow_frame,
        local_shadow_frame,
        world_lights,
    }
}
