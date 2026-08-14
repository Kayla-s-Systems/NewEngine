use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::RenderApi;
use newengine_scene::Scene;

use crate::engine_bounds::EngineBoundsSnap;
use crate::gameplay::{
    consume_player_transient_input, run_schedule_with_physics_mode_and_telemetry_for_frame,
    GameRunMode, PhysicsIntegrationMode,
};
use crate::scene_bridge::EngineViewInput;

use super::super::controller::RuntimeRenderController;
use super::frame_types::WorldFrameState;
use super::input::ViewportInputSnap;
use super::{readiness, scene};

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

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

        let view_frame = scene.run_frame(self.frame.frame_index, |world| {
            let _authority_frame = scene_bridge.authority_bridge().publish_frame(
                world,
                self.frame.frame_index,
                "render.world_tick",
            );

            let world_playable = readiness::update_world_activation_gate(
                self,
                r,
                world,
                play_mode,
                self.frame.frame_index,
            );
            let gate_released_waiting_activation = world
                .resource::<crate::gameplay::WorldActivationState>()
                .map(|gate| gate.is_ready() && !gate.is_active() && !gate.is_preview_ready())
                .unwrap_or(false);

            let effective_play_mode = if gate_released_waiting_activation {
                if let Some(gate) = world.resource_mut::<crate::gameplay::WorldActivationState>() {
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
            let mut engine_view_input = EngineViewInput::from(input);
            scene_bridge.prepare_engine_runtime_input(
                world,
                engine_view_input.clone(),
                effective_play_mode,
                self.frame.frame_index,
            );
            // The view request was consumed in the pre-simulation input phase. Keeping
            // it in the render-phase packet would cycle camera modes twice in one frame.
            engine_view_input.camera_view = newengine_input_actions_api::CameraViewRequest::None;

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
                    crate::WorldRuntimeFrame {
                        frame_index: self.frame.frame_index,
                        dt,
                        runtime_active: effective_play_mode.is_runtime(),
                        streaming_enabled: runtime_profile.use_runtime_terrain_streaming(),
                        environment_cycle_enabled: runtime_profile.tick_sky_cycle(),
                    },
                );
            }

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
                    let telemetry_interval = crate::env_config::var_u64(
                        "NEWENGINE_SIM_TELEMETRY_INTERVAL_TICKS",
                        120,
                        1,
                        60_000,
                    );
                    let slow_tick_ms =
                        crate::env_config::var_f32("NEWENGINE_SIM_SLOW_TICK_MS", 4.0, 0.25, 1000.0);
                    for step_index in 0..fixed_step_count {
                        let remaining_after_step = u64::from(fixed_step_count - step_index - 1);
                        let simulation_tick = fixed_tick.saturating_sub(remaining_after_step);
                        world.insert_resource(crate::gameplay::PhysicsRuntimeFrameIndex(
                            simulation_tick,
                        ));
                        let detailed_telemetry = simulation_tick <= 4
                            || simulation_tick.is_multiple_of(telemetry_interval);
                        let tick_started = Instant::now();
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
                        let tick_elapsed_ms = tick_started.elapsed().as_secs_f32() * 1000.0;
                        if detailed_telemetry || tick_elapsed_ms >= slow_tick_ms {
                            emit_simulation_tick_profile(
                                simulation_tick,
                                tick_elapsed_ms,
                                fixed_dt,
                                fixed_step_count,
                                step_index,
                                physics_mode,
                                slow_tick_ms,
                            );
                        }
                        consume_player_transient_input(world);
                    }
                } else {
                    log_physics_skip_once();
                }
            }

            // Gameplay systems may change modal UI state during the fixed step.
            // Synchronize the generic capture contract before camera/input resolution.
            self.frame.gameplay_ui.sync_modal_state(world);

            let bounds = scene::scene_bounds_world(world).unwrap_or_else(scene::default_bounds);
            let bounds = EngineBoundsSnap::new(bounds.center, bounds.radius);
            let sel_bounds = scene::selection_bounds_world(world, selection)
                .map(|bounds| EngineBoundsSnap::new(bounds.center, bounds.radius));
            let frame = scene_bridge.resolve_engine_view_frame(
                world,
                &viewport_bridge,
                engine_view_input,
                play_mode,
                effective_play_mode,
                world_playable,
                self.frame.frame_index,
                dt,
                vp_w,
                vp_h,
                bounds,
                sel_bounds,
            );
            self.frame.last_play_mode = effective_play_mode;
            self.frame.last_camera_snapshot = Some(frame.camera_snapshot);
            self.viewport.last_aspect = frame.view.aspect;
            self.viewport.last_vp_w = vp_w;
            self.viewport.last_vp_h = vp_h;
            frame
        });

        if activate_world_play_after_frame {
            self.bridges.scene.activate_profile_play_now();
        }

        WorldFrameState { view_frame }
    }
}

const PROFILER_SAMPLE_TOPIC: &str = "newengine.diagnostics.profiler.sample.v1";

#[inline]
fn emit_simulation_tick_profile(
    simulation_tick: u64,
    elapsed_ms: f32,
    fixed_dt: f32,
    fixed_step_count: u32,
    step_index: u32,
    physics_mode: PhysicsIntegrationMode,
    slow_tick_ms: f32,
) {
    let frame_budget_ms = fixed_dt.max(0.000_001) * 1000.0;
    let payload = serde_json::json!({
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
    });
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
