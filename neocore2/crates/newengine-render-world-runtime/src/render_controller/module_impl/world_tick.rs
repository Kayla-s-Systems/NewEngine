use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::RenderApi;
use newengine_scene::Scene;

use newengine_bounds::EngineBoundsSnap;
use newengine_gameplay_world_runtime::gameplay::{
    capture_player_fixed_poses, consume_player_transient_input, publish_player_render_poses,
    run_schedule_with_physics_mode_and_telemetry_for_frame, GameRunMode, GameplayExecutionPhase,
    GameplayFrame, PhysicsIntegrationMode,
};
use newengine_scene_bridge_runtime::scene_bridge::EngineViewInput;

use super::super::controller::RuntimeRenderController;
use super::frame_types::WorldFrameState;
use super::input::ViewportInputSnap;
use super::{readiness, scene};

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default)]
struct WorldControllerPhaseTiming {
    authority_ms: f32,
    activation_ms: f32,
    input_ms: f32,
    world_runtime_ms: f32,
    simulation_ms: f32,
    render_pose_ms: f32,
    view_resolve_ms: f32,
}

impl RuntimeRenderController {
    pub(super) fn tick_world_for_render(
        &mut self,
        r: &mut dyn RenderApi,
        physics_api: Option<&PhysicsApiRef>,
        thread_pool: Option<&newengine_core::ThreadPoolHandle>,
        job_events: Option<&newengine_core::EventHub>,
        scene: &mut Scene,
        input: &ViewportInputSnap,
        play_mode: GameRunMode,
        dt: f32,
        fixed_dt: f32,
        fixed_alpha: f32,
        fixed_step_count: u32,
        fixed_tick: u64,
        pause_world: bool,
        _aspect: f32,
        vp_w: u32,
        vp_h: u32,
    ) -> WorldFrameState {
        let mut activate_world_play_after_frame = false;
        let viewport_bridge = self.bridges.viewport.clone();
        let scene_bridge = self.bridges.scene.clone();
        let selection = scene_bridge.selection();
        let mut controller_timing = WorldControllerPhaseTiming::default();

        let mut view_frame = scene.run_frame(self.frame.frame_index, |world| {
            let phase_started = Instant::now();
            let _authority_frame = scene_bridge.authority_bridge().publish_frame(
                world,
                self.frame.frame_index,
                "render.world_tick",
            );
            controller_timing.authority_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

            let phase_started = Instant::now();
            let world_playable = readiness::update_world_activation_gate(
                self,
                r,
                world,
                play_mode,
                self.frame.frame_index,
            );
            let gate_released_waiting_activation = world
                .resource::<newengine_gameplay_world_runtime::gameplay::WorldActivationState>()
                .map(|gate| gate.is_ready() && !gate.is_active() && !gate.is_preview_ready())
                .unwrap_or(false);

            let effective_play_mode = if gate_released_waiting_activation {
                if let Some(gate) = world.resource_mut::<newengine_gameplay_world_runtime::gameplay::WorldActivationState>() {
                    gate.mark_active();
                }
                activate_world_play_after_frame = true;
                GameRunMode::Play
            } else if world_playable {
                play_mode
            } else {
                GameRunMode::Staging
            };

            let runtime_profile = self.runtime_profile().clone();
            // Unified Editor Mode keeps the live authored world loaded but gives camera
            // ownership to the generic viewport navigator. Staging is the camera-only
            // control policy: it never routes WASD to the possessed PlayerActor.
            let camera_play_mode = if self.editor_viewport.is_active() {
                GameRunMode::Staging
            } else {
                effective_play_mode
            };
            controller_timing.activation_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

            let phase_started = Instant::now();
            let mut engine_view_input = EngineViewInput::from(input);
            scene_bridge.prepare_engine_runtime_input(
                world,
                engine_view_input.clone(),
                camera_play_mode,
                self.frame.frame_index,
            );

            // UI/menu actions are render-input concerns, not fixed-simulation concerns.
            // Run this phase unconditionally so a key edge such as M is consumed even when
            // the accumulator produces zero fixed steps for this presented frame.
            self.frame.gameplay_systems.run_phase(
                GameplayExecutionPhase::FrameInput,
                world,
                GameplayFrame { dt, fixed_tick },
            );
            self.frame.gameplay_ui.sync_modal_state(world);

            // The view request was consumed in the pre-simulation input phase. Keeping
            // it in the render-phase packet would cycle camera modes twice in one frame.
            engine_view_input.camera_view = newengine_input_actions_api::CameraViewRequest::None;
            controller_timing.input_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

            let phase_started = Instant::now();
            {
                let prims_lock = scene_bridge.primitives();
                let mut prims = prims_lock.write();
                let mats_lock = scene_bridge.materials();
                let mats = mats_lock.read();
                self.frame.world_runtime.tick_frame(
                    world,
                    &mut prims,
                    &mats,
                    thread_pool,
                    newengine_world_runtime_api::WorldRuntimeFrame {
                        frame_index: self.frame.frame_index,
                        dt,
                        runtime_active: effective_play_mode.is_runtime(),
                        streaming_enabled: runtime_profile.use_runtime_terrain_streaming(),
                        environment_cycle_enabled: runtime_profile.tick_sky_cycle(),
                    },
                );
            }
            controller_timing.world_runtime_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

            let phase_started = Instant::now();
            if effective_play_mode.runs_physics() && !pause_world {
                let physics_mode = if runtime_profile.use_service_physics() {
                    if physics_api.is_some() {
                        Some(PhysicsIntegrationMode::ServiceBackend)
                    } else {
                        log_service_physics_unavailable_once();
                        None
                    }
                } else if runtime_profile.use_fallback_ecs_physics() {
                    // This is an explicit profile policy path, not a hidden fallback.
                    Some(PhysicsIntegrationMode::EcsFallback)
                } else {
                    None
                };
                if let Some(physics_mode) = physics_mode {
                    let publish_sim_job = |event: newengine_task_api::EngineTaskEvent| {
                        let job_event = newengine_task_api::EngineTaskEnvelopeV1::new(
                            event.clone(),
                            newengine_task_api::TaskExecutorKind::SimulationInternalParallelism,
                            "simulation-job-batch",
                        );
                        if let Ok(payload) = serde_json::to_vec(&event) {
                            let _ = newengine_plugin_host::host_context::publish_event(
                                newengine_task_api::ENGINE_TASK_EVENT_TOPIC_V1,
                                &payload,
                            );
                        }
                        if let Ok(payload) = serde_json::to_vec(&job_event) {
                            let _ = newengine_plugin_host::host_context::publish_event(
                                newengine_task_api::ENGINE_TASK_ENVELOPE_TOPIC_V1,
                                &payload,
                            );
                        }
                        if let Some(events) = job_events {
                            let _ = events.publish(event);
                            let _ = events.publish(job_event);
                        }
                    };
                    let sim_telemetry =
                        newengine_sim::SimulationJobTelemetry::new(&publish_sim_job);
                    let fixed_dt = fixed_dt.max(0.000_001);
                    let simulation_policy = newengine_runtime_policy::simulation_runtime_policy();
                    let telemetry_interval = simulation_policy.telemetry_interval_ticks;
                    let slow_tick_ms = simulation_policy.slow_tick_ms;
                    for step_index in 0..fixed_step_count {
                        let remaining_after_step = u64::from(fixed_step_count - step_index - 1);
                        let simulation_tick = fixed_tick.saturating_sub(remaining_after_step);
                        world.insert_resource(newengine_gameplay_world_runtime::gameplay::PhysicsRuntimeFrameIndex(
                            simulation_tick,
                        ));
                        let detailed_telemetry = simulation_tick <= 4
                            || simulation_tick.is_multiple_of(telemetry_interval);
                        let tick_started = Instant::now();
                        let schedule_timing =
                            run_schedule_with_physics_mode_and_telemetry_for_frame(
                                &mut self.frame.sim_schedule,
                                &mut self.frame.gameplay_content,
                                &self.frame.gameplay_systems,
                                &self.frame.gameplay_physics_queries,
                                world,
                                fixed_dt,
                                simulation_tick,
                                physics_api,
                                physics_mode,
                                detailed_telemetry.then_some(&sim_telemetry),
                                thread_pool,
                            );
                        // Capture the completed fixed pose before the next simulation step. The
                        // render stage interpolates this pair using Frame::fixed_alpha.
                        let capture_pose_started = Instant::now();
                        capture_player_fixed_poses(world, simulation_tick);
                        let capture_fixed_pose_ms =
                            capture_pose_started.elapsed().as_secs_f32() * 1000.0;
                        let tick_elapsed_ms = tick_started.elapsed().as_secs_f32() * 1000.0;
                        let physics_timing = world
                            .resource::<newengine_gameplay_world_runtime::gameplay::PhysicsStepTimingTelemetry>()
                            .copied();
                        if detailed_telemetry || tick_elapsed_ms >= slow_tick_ms {
                            emit_simulation_tick_profile(
                                simulation_tick,
                                tick_elapsed_ms,
                                fixed_dt,
                                fixed_step_count,
                                step_index,
                                physics_mode,
                                slow_tick_ms,
                                physics_timing,
                                schedule_timing,
                                capture_fixed_pose_ms,
                            );
                        }
                        consume_player_transient_input(world);
                    }
                } else {
                    log_physics_skip_once();
                }
            }
            controller_timing.simulation_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

            // Gameplay systems may change modal UI state during the fixed step.
            // Synchronize the generic capture contract before camera/input resolution.
            let phase_started = Instant::now();
            self.frame.gameplay_ui.sync_modal_state(world);

            // Render presentation is decoupled from fixed-step Transform. Camera and player
            // visuals consume this same interpolated pose, eliminating 60 Hz stepping at
            // higher render rates without mutating simulation/physics state.
            publish_player_render_poses(world, fixed_alpha, dt);
            controller_timing.render_pose_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

            let phase_started = Instant::now();
            let bounds = scene::scene_bounds_world(world).unwrap_or_else(scene::default_bounds);
            let bounds = EngineBoundsSnap::new(bounds.center, bounds.radius);
            let sel_bounds = scene::selection_bounds_world(world, selection)
                .map(|bounds| EngineBoundsSnap::new(bounds.center, bounds.radius));
            let frame = scene_bridge.resolve_engine_view_frame(
                world,
                &viewport_bridge,
                engine_view_input,
                play_mode,
                camera_play_mode,
                world_playable,
                self.frame.frame_index,
                dt,
                vp_w,
                vp_h,
                bounds,
                sel_bounds,
            );
            self.frame.last_play_mode = effective_play_mode;
            controller_timing.view_resolve_ms = phase_started.elapsed().as_secs_f32() * 1000.0;
            frame
        });

        if self.editor_viewport.is_active() {
            let world_bounds =
                scene::scene_bounds_world(scene.world()).unwrap_or_else(scene::default_bounds);
            let selection_bounds = scene::selection_bounds_world(scene.world(), selection);
            newengine_scene_bridge_runtime::editor_viewport_adapter::apply_camera_projection(
                &mut self.editor_viewport,
                &mut view_frame,
                world_bounds.center,
                world_bounds.radius,
                selection_bounds.map(|bounds| bounds.center),
                selection_bounds.map(|bounds| bounds.radius),
                input.wheel_y,
                [vp_w, vp_h],
            );
        }
        self.frame.last_camera_snapshot = Some(view_frame.camera_snapshot);
        self.viewport.last_aspect = view_frame.view.aspect;
        self.viewport.last_vp_w = vp_w;
        self.viewport.last_vp_h = vp_h;

        if let Some(timing) = scene
            .world()
            .resource::<newengine_scene::SceneFrameTimingTelemetry>()
        {
            if timing.total_ms >= 8.0 || timing.frame_index.is_multiple_of(120) {
                let physics_timing = scene
                    .world()
                    .resource::<newengine_gameplay_world_runtime::gameplay::PhysicsStepTimingTelemetry>()
                    .copied();
                emit_scene_frame_profile(*timing, physics_timing, controller_timing);
            }
        }

        if activate_world_play_after_frame {
            self.bridges.scene.activate_profile_play_now();
        }

        WorldFrameState { view_frame }
    }
}

const PROFILER_SAMPLE_TOPIC: &str = "newengine.diagnostics.profiler.sample.v1";

#[inline]
fn emit_scene_frame_profile(
    timing: newengine_scene::SceneFrameTimingTelemetry,
    physics_timing: Option<newengine_gameplay_world_runtime::gameplay::PhysicsStepTimingTelemetry>,
    controller: WorldControllerPhaseTiming,
) {
    let mut payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "scene.frame",
        "source": "render.world_tick",
        "name": "scene derived-state frame",
        "lane": "scene",
        "priority": "interactive",
        "dependency_group": format!("scene.frame.{}", timing.frame_index),
        "frame_index": timing.frame_index,
        "elapsed_ms": timing.total_ms,
        "pre_derive_ms": timing.pre_derive_ms,
        "controller_ms": timing.controller_ms,
        "post_derive_ms": timing.post_derive_ms,
        "controller_authority_ms": controller.authority_ms,
        "controller_activation_ms": controller.activation_ms,
        "controller_input_ms": controller.input_ms,
        "controller_world_runtime_ms": controller.world_runtime_ms,
        "controller_simulation_ms": controller.simulation_ms,
        "controller_render_pose_ms": controller.render_pose_ms,
        "controller_view_resolve_ms": controller.view_resolve_ms,
        "slow": timing.total_ms >= 8.0,
    });
    if let (Some(object), Some(physics)) = (payload.as_object_mut(), physics_timing) {
        object.insert(
            "physics_input_build_ms".to_owned(),
            serde_json::json!(physics.input_build_ms),
        );
        object.insert(
            "physics_backend_step_ms".to_owned(),
            serde_json::json!(physics.backend_step_ms),
        );
        object.insert(
            "physics_output_apply_ms".to_owned(),
            serde_json::json!(physics.output_apply_ms),
        );
        object.insert(
            "physics_bodies".to_owned(),
            serde_json::json!(physics.bodies),
        );
        object.insert(
            "physics_colliders".to_owned(),
            serde_json::json!(physics.colliders),
        );
        object.insert(
            "physics_commands".to_owned(),
            serde_json::json!(physics.commands),
        );
        object.insert(
            "physics_queries".to_owned(),
            serde_json::json!(physics.queries),
        );
        object.insert(
            "physics_pose_updates".to_owned(),
            serde_json::json!(physics.pose_updates),
        );
        object.insert(
            "physics_velocity_updates".to_owned(),
            serde_json::json!(physics.velocity_updates),
        );
        object.insert(
            "physics_events".to_owned(),
            serde_json::json!(physics.events),
        );
        object.insert(
            "physics_query_hits".to_owned(),
            serde_json::json!(physics.query_hits),
        );
    }
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        let _ = newengine_plugin_host::emit_plugin_event(PROFILER_SAMPLE_TOPIC, &bytes);
    }
}

#[inline]
fn emit_simulation_tick_profile(
    simulation_tick: u64,
    elapsed_ms: f32,
    fixed_dt: f32,
    fixed_step_count: u32,
    step_index: u32,
    physics_mode: PhysicsIntegrationMode,
    slow_tick_ms: f32,
    physics_timing: Option<newengine_gameplay_world_runtime::gameplay::PhysicsStepTimingTelemetry>,
    schedule_timing: newengine_gameplay_world_runtime::gameplay::SimulationScheduleTiming,
    capture_fixed_pose_ms: f32,
) {
    let frame_budget_ms = fixed_dt.max(0.000_001) * 1000.0;
    let mut payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "simulation.tick",
        "source": "render.world_tick",
        "name": "simulation fixed tick",
        "lane": "simulation",
        "priority": "interactive",
        "dependency_group": format!("simulation.tick.{simulation_tick}"),
        "frame_index": simulation_tick,
        "elapsed_ms": elapsed_ms,
        "budget_ms": slow_tick_ms,
        "frame_budget_ms": frame_budget_ms,
        "exceeded_frame_budget": elapsed_ms > frame_budget_ms,
        "slow": elapsed_ms >= slow_tick_ms,
        "fixed_step_count": fixed_step_count,
        "step_index": step_index,
        "catch_up": fixed_step_count > 1,
        "physics_mode": format!("{physics_mode:?}"),
        "schedule_total_ms": schedule_timing.total_ms,
        "schedule_input_ms": schedule_timing.input_ms,
        "schedule_controllers_ms": schedule_timing.controllers_ms,
        "schedule_apply_intents_ms": schedule_timing.apply_intents_ms,
        "schedule_content_install_ms": schedule_timing.content_install_ms,
        "schedule_capability_ensure_ms": schedule_timing.capability_ensure_ms,
        "schedule_before_physics_ms": schedule_timing.before_physics_ms,
        "schedule_physics_ms": schedule_timing.physics_ms,
        "schedule_after_physics_ms": schedule_timing.after_physics_ms,
        "schedule_derived_ms": schedule_timing.derived_ms,
        "schedule_after_derived_ms": schedule_timing.after_derived_ms,
        "schedule_capability_dispatch_ms": schedule_timing.capability_dispatch_ms,
        "schedule_capability_requested": schedule_timing.capability_requested,
        "schedule_capability_executed": schedule_timing.capability_executed,
        "schedule_capability_missing": schedule_timing.capability_missing,
        "schedule_capability_failed": schedule_timing.capability_failed,
        "schedule_animation_state_ms": schedule_timing.animation_state_ms,
        "capture_fixed_pose_ms": capture_fixed_pose_ms,
    });
    if let (Some(object), Some(timing)) = (payload.as_object_mut(), physics_timing) {
        object.insert(
            "physics_packet_frame_index".to_owned(),
            serde_json::json!(timing.frame_index),
        );
        object.insert(
            "physics_packet_fixed_tick".to_owned(),
            serde_json::json!(timing.fixed_tick),
        );
        object.insert(
            "physics_input_build_ms".to_owned(),
            serde_json::json!(timing.input_build_ms),
        );
        object.insert(
            "physics_backend_step_ms".to_owned(),
            serde_json::json!(timing.backend_step_ms),
        );
        object.insert(
            "physics_output_apply_ms".to_owned(),
            serde_json::json!(timing.output_apply_ms),
        );
        object.insert(
            "physics_bodies".to_owned(),
            serde_json::json!(timing.bodies),
        );
        object.insert(
            "physics_colliders".to_owned(),
            serde_json::json!(timing.colliders),
        );
        object.insert(
            "physics_commands".to_owned(),
            serde_json::json!(timing.commands),
        );
        object.insert(
            "physics_queries".to_owned(),
            serde_json::json!(timing.queries),
        );
        object.insert(
            "physics_pose_updates".to_owned(),
            serde_json::json!(timing.pose_updates),
        );
        object.insert(
            "physics_velocity_updates".to_owned(),
            serde_json::json!(timing.velocity_updates),
        );
        object.insert(
            "physics_events".to_owned(),
            serde_json::json!(timing.events),
        );
        object.insert(
            "physics_query_hits".to_owned(),
            serde_json::json!(timing.query_hits),
        );
    }
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        let _ = newengine_plugin_host::emit_plugin_event(PROFILER_SAMPLE_TOPIC, &bytes);
    }
}

static GPU_SAFE_PHYSICS_SKIP_LOGGED: AtomicBool = AtomicBool::new(false);
static SERVICE_PHYSICS_UNAVAILABLE_LOGGED: AtomicBool = AtomicBool::new(false);

fn log_service_physics_unavailable_once() {
    if SERVICE_PHYSICS_UNAVAILABLE_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        newengine_ulog_api::ulog::warn!(
            "render world tick: engine.physics provider unavailable; physics step skipped because no explicit ECS fallback profile policy is active"
        );
        newengine_core::crash::record_breadcrumb(
            "render world tick: missing physics backend skipped; no hidden fallback constructed",
        );
    }
}

fn log_physics_skip_once() {
    if GPU_SAFE_PHYSICS_SKIP_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        newengine_ulog_api::ulog::warn!(
            "render world tick: physics schedule skipped by explicit profile policy or missing provider; no hidden ECS fallback constructed"
        );
        newengine_core::crash::record_breadcrumb(
            "render world tick: physics schedule skipped without hidden fallback",
        );
    }
}

impl From<&ViewportInputSnap> for EngineViewInput {
    #[inline]
    fn from(input: &ViewportInputSnap) -> Self {
        Self {
            dx_px: input.dx_px,
            dy_px: input.dy_px,
            wheel_y: input.wheel_y,
            active: input.active,
            look_drag: input.look_drag,
            pan_drag: input.pan_drag,
            ui_busy: input.ui_busy,
            fly_rmb: input.fly_rmb,
            sampling_alive: input.sampling_alive,
            camera_navigation_gated: input.camera_navigation_gated,
            gameplay_movement_gated: input.gameplay_movement_gated,
            move_mask: input.move_mask,
            speed_scalar: input.speed_scalar,
            camera_view: input.camera_view,
            gameplay_actions: input.actions.command_actions(),
        }
    }
}
