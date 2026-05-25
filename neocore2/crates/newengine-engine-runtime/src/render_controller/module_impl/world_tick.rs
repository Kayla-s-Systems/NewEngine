use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::RenderApi;
use newengine_scene::Scene;

use crate::engine_bounds::EngineBoundsSnap;
use crate::scene_bridge::EngineViewInput;
use crate::gameplay::{run_schedule_with_physics_mode_and_telemetry, GameRunMode, PhysicsIntegrationMode};

use super::frame_types::WorldFrameState;
use super::input::ViewportInputSnap;
use super::super::controller::RuntimeRenderController;
use super::{readiness, scene};

use std::sync::atomic::{AtomicBool, Ordering};

impl RuntimeRenderController {
    pub(super) fn tick_world_for_render(
        &mut self,
        r: &mut dyn RenderApi,
        physics_api: Option<&PhysicsApiRef>,
        job_system: Option<&newengine_core::JobSystemHandle>,
        job_events: Option<&newengine_core::EventHub>,
        scene: &mut Scene,
        input: &ViewportInputSnap,
        play_mode: GameRunMode,
        dt: f32,
        pause_world: bool,
        _aspect: f32,
        vp_w: u32,
        vp_h: u32,
    ) -> WorldFrameState {
        let mut activate_game_ready_play_after_frame = false;
        let viewport_bridge = self.bridges.viewport.clone();
        let scene_bridge = self.bridges.scene.clone();
        let selection = scene_bridge.selection();

        let view_frame = scene.run_frame(self.frame.frame_index, |world| {
            let _authority_frame = scene_bridge
                .authority_bridge()
                .publish_frame(world, self.frame.frame_index, "render.world_tick");

            let world_playable = readiness::update_game_ready_launch_gate(
                self,
                r,
                world,
                play_mode,
                self.frame.frame_index,
            );
            let gate_released_waiting_activation = world
                .resource::<crate::gameplay::GameReadyWorldLaunchGate>()
                .map(|gate| gate.is_released() && !gate.is_play_activated())
                .unwrap_or(false);

            let effective_play_mode = if gate_released_waiting_activation {
                if let Some(gate) = world.resource_mut::<crate::gameplay::GameReadyWorldLaunchGate>() {
                    gate.mark_play_activated();
                }
                activate_game_ready_play_after_frame = true;
                GameRunMode::Play
            } else if world_playable {
                play_mode
            } else {
                GameRunMode::Staging
            };

            let runtime_profile = self.runtime_profile().clone();

            if effective_play_mode.is_runtime() {
                if runtime_profile.use_runtime_terrain_streaming() {
                    let mats_lock = scene_bridge.materials();
                    let mats = mats_lock.read();
                    crate::scene_bridge::tick_game_ready_streaming_terrain(world, &mats, job_system);
                } else {
                    log_streaming_skip_once();
                }
            }
            if runtime_profile.tick_sky_cycle() {
                crate::scene_bridge::tick_game_ready_sky_cycle(world, dt);
            }

            if effective_play_mode.runs_physics() && !pause_world {
                if runtime_profile.use_service_physics() || runtime_profile.use_fallback_ecs_physics() {
                    world.insert_resource(crate::gameplay::PhysicsRuntimeFrameIndex(self.frame.frame_index));
                    let physics_mode = if runtime_profile.use_service_physics() {
                        if physics_api.is_some() {
                            PhysicsIntegrationMode::ServiceBackend
                        } else {
                            log_service_physics_downgraded_once();
                            PhysicsIntegrationMode::EcsFallback
                        }
                    } else {
                        PhysicsIntegrationMode::EcsFallback
                    };
                    let publish_sim_job = |event: newengine_jobs_api::EngineTaskEvent| {
                        let job_event = newengine_jobs_api::EngineJobEventV1::new(
                            event.clone(),
                            newengine_jobs_api::JobExecutorKind::SimulationInternalParallelism,
                            "simulation-job-batch",
                        );
                        if let Ok(payload) = serde_json::to_vec(&event) {
                            let _ = newengine_plugin_host::host_context::publish_event(
                                newengine_jobs_api::ENGINE_TASK_EVENT_TOPIC_V1,
                                &payload,
                            );
                        }
                        if let Ok(payload) = serde_json::to_vec(&job_event) {
                            let _ = newengine_plugin_host::host_context::publish_event(
                                newengine_jobs_api::ENGINE_JOB_EVENT_TOPIC_V1,
                                &payload,
                            );
                        }
                        if let Some(events) = job_events {
                            let _ = events.publish(event);
                            let _ = events.publish(job_event);
                        }
                    };
                    let sim_telemetry = newengine_sim::SimulationJobTelemetry::new(&publish_sim_job);
                    run_schedule_with_physics_mode_and_telemetry(
                        &mut self.frame.sim_schedule,
                        world,
                        dt,
                        physics_api,
                        physics_mode,
                        Some(&sim_telemetry),
                    );
                } else {
                    log_physics_skip_once();
                }
            }

            let bounds = scene::scene_bounds_world(world).unwrap_or_else(scene::default_bounds);
            let bounds = EngineBoundsSnap::new(bounds.center, bounds.radius);
            let sel_bounds = scene::selection_bounds_world(world, selection)
                .map(|bounds| EngineBoundsSnap::new(bounds.center, bounds.radius));
            let frame = scene_bridge.resolve_engine_view_frame(
                world,
                &viewport_bridge,
                EngineViewInput::from(input),
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

        if activate_game_ready_play_after_frame {
            self.bridges.scene.activate_profile_play_now();
        }

        WorldFrameState { view_frame }
    }
}

static GPU_SAFE_PHYSICS_SKIP_LOGGED: AtomicBool = AtomicBool::new(false);
static SERVICE_PHYSICS_DOWNGRADED_LOGGED: AtomicBool = AtomicBool::new(false);
static GPU_SAFE_STREAMING_SKIP_LOGGED: AtomicBool = AtomicBool::new(false);


fn log_service_physics_downgraded_once() {
    if SERVICE_PHYSICS_DOWNGRADED_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::warn!(
            "render world tick: engine.physics provider unavailable; downgraded simulation to ECS fallback for this run; scene launch and public Play are not blocked by physics backend presence or type"
        );
        newengine_core::crash::record_breadcrumb(
            "render world tick: missing physics backend downgraded to ECS fallback without blocking launch".to_owned(),
        );
    }
}

fn log_physics_skip_once() {
    if GPU_SAFE_PHYSICS_SKIP_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::warn!(
            "render world tick: native physics schedule skipped by conservative GPU profile; change plugins.newengine.engine_runtime.render.runtime_profile.world.service_physics to 'enabled' to test the original physics path"
        );
        newengine_core::crash::record_breadcrumb(
            "render world tick: conservative profile skipped native physics schedule".to_owned(),
        );
    }
}

fn log_streaming_skip_once() {
    if GPU_SAFE_STREAMING_SKIP_LOGGED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        log::warn!(
            "render world tick: runtime terrain streaming skipped by conservative GPU profile; change plugins.newengine.engine_runtime.render.runtime_profile.world.runtime_terrain_streaming to 'enabled' to test the original streaming path"
        );
        newengine_core::crash::record_breadcrumb(
            "render world tick: conservative profile skipped runtime terrain streaming".to_owned(),
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
            move_mask: input.move_mask,
            speed_scalar: input.speed_scalar,
            camera_view: input.camera_view,
        }
    }
}
