use newengine_camera::{
    CameraChannel, CameraChannelState, CameraViewport, EditorNavController, EditorNavMode,
};
use newengine_camera_runtime::{
    step_camera_nav, BoundsSphere as CamBoundsSphere, CameraManagerResource,
    CameraNavFrameRequest, CameraNavInput, CameraNavParams, CameraRuntimeService,
    CameraRuntimeServiceConfig, CameraRuntimeWorldState,
};
use newengine_core::render::RenderApi;
use newengine_math::Vec2;
use newengine_scene::Scene;

use super::frame_types::WorldFrameState;
use super::input::ViewportInputSnap;
use super::super::controller::RuntimeRenderController;
use super::{readiness, scene};
use crate::gameplay::{
    capture_runtime_world_snapshot, first_player, restore_runtime_world_snapshot, run_schedule,
    EditorPlayMode, FpsDemoRules,
};

impl RuntimeRenderController {
    pub(super) fn tick_world_for_render(
        &mut self,
        r: &mut dyn RenderApi,
        scene: &mut Scene,
        input: &ViewportInputSnap,
        play_mode: EditorPlayMode,
        dt: f32,
        _aspect: f32,
        vp_w: u32,
        vp_h: u32,
    ) -> WorldFrameState {
        let mut nav_input = camera_nav_input(input, play_mode);
        let mut activate_game_ready_play_after_frame = false;

        let (camera_frame, effective_play_mode, world_playable) =
            scene.run_frame(self.frame_index, |world| {
                let cam_id = world
                    .resource::<newengine_scene::SceneState>()
                    .and_then(|s| s.active_camera.or(s.root))
                    .unwrap_or_default();

                CameraRuntimeService::ensure_manager_resource(world);

                let world_playable = readiness::update_game_ready_launch_gate(
                    self,
                    r,
                    world,
                    play_mode,
                    self.frame_index,
                );
                let gate_released_waiting_activation = world
                    .resource::<crate::gameplay::GameReadyWorldLaunchGate>()
                    .map(|gate| gate.is_released() && !gate.is_play_activated())
                    .unwrap_or(false);

                let effective_play_mode = if gate_released_waiting_activation {
                    if let Some(gate) =
                        world.resource_mut::<crate::gameplay::GameReadyWorldLaunchGate>()
                    {
                        gate.mark_play_activated();
                    }
                    activate_game_ready_play_after_frame = true;
                    EditorPlayMode::Play
                } else if world_playable {
                    play_mode
                } else {
                    EditorPlayMode::Edit
                };

                let existing_nav_mode = world
                    .get::<EditorNavController>(cam_id)
                    .map(|ctrl| ctrl.mode)
                    .unwrap_or(EditorNavMode::Orbit);
                let player = first_player(world);
                let gate_blocked = play_mode.is_runtime() && !world_playable;

                let suppress_editor_nav = {
                    let manager = world
                        .resource_mut::<CameraManagerResource>()
                        .expect("camera manager resource inserted");
                    manager.advance(dt);
                    manager.sync_world_state(CameraRuntimeWorldState {
                        editor_nav_mode: existing_nav_mode,
                        runtime_requested: play_mode.is_runtime(),
                        public_runtime_active: effective_play_mode.is_runtime(),
                        wants_direct_player_control: effective_play_mode
                            .wants_direct_player_control(),
                        gate_blocked,
                        player,
                    });
                    !manager.wants_navigation_input()
                };

                self.sync_play_mode_transition(world, cam_id, effective_play_mode);
                let service_config = camera_runtime_service_config(world);
                CameraRuntimeService::apply_pending_director_requests(
                    world,
                    cam_id,
                    service_config,
                );
                self.apply_runtime_input(world, input, effective_play_mode, service_config);

                if effective_play_mode.runs_physics() {
                    run_schedule(&mut self.sim_schedule, world, dt);
                }

                let bounds = scene::scene_bounds_world(world).unwrap_or_else(scene::default_bounds);
                let sel_bounds = scene::selection_bounds_world(world, self.scene_bridge.selection());
                let params = CameraNavParams {
                    dt,
                    viewport: CameraViewport::from_size(vp_w, vp_h),
                    channel: CameraChannelState::dominant(if effective_play_mode.is_runtime() {
                        CameraChannel::Gameplay
                    } else {
                        CameraChannel::Editor
                    }),
                    bounds: CamBoundsSphere {
                        center: bounds.center,
                        radius: bounds.radius,
                    },
                    selection_bounds: sel_bounds.map(|b| CamBoundsSphere {
                        center: b.center,
                        radius: b.radius,
                    }),
                };

                let frame_req = CameraNavFrameRequest {
                    seq: self.viewport_bridge.read_frame_request(),
                    all: self.viewport_bridge.read_frame_all(),
                };

                if suppress_editor_nav || effective_play_mode.wants_direct_player_control() {
                    nav_input.active = false;
                    nav_input.look_drag = false;
                    nav_input.pan_drag = false;
                    nav_input.fly_rmb = false;
                    nav_input.move_mask = 0;
                    nav_input.wheel_y = 0.0;
                }

                let out = step_camera_nav(
                    &mut self.camera_nav,
                    world,
                    cam_id,
                    &mut nav_input,
                    params,
                    frame_req,
                );

                let camera_frame = if let Some(manager) = world.resource_mut::<CameraManagerResource>() {
                    manager.sync_editor_mode_from_controller(out.controller.mode);
                    manager.set_last_cursor(out.cursor);
                    let frame = manager.resolve_camera_frame(out.frame, dt);
                    self.overlay_metrics.record_camera_report(manager.report());
                    frame
                } else {
                    out.frame
                };
                self.projection = camera_frame.projection;
                self.last_aspect = camera_frame.viewport.aspect();
                self.last_vp_w = vp_w;
                self.last_vp_h = vp_h;

                (camera_frame, effective_play_mode, world_playable)
            });

        if activate_game_ready_play_after_frame {
            self.scene_bridge.activate_game_ready_play_now();
        }

        WorldFrameState {
            camera_frame,
            effective_play_mode,
            world_playable,
            nav_input,
        }
    }

    fn sync_play_mode_transition(
        &mut self,
        world: &mut newengine_ecs::World,
        cam_id: newengine_ecs::EntityId,
        effective_play_mode: EditorPlayMode,
    ) {
        if self.last_play_mode == effective_play_mode {
            return;
        }

        if !self.last_play_mode.is_runtime() && effective_play_mode.is_runtime() {
            self.runtime_session = Some(capture_runtime_world_snapshot(world));
        }

        if self.last_play_mode.wants_direct_player_control() {
            if let Some(player) = first_player(world) {
                CameraRuntimeService::clear_player_input(world, player);
            }
            if let Some(snapshot) = self.play_session.take() {
                let _ = world.insert(snapshot.cam_id, snapshot.rig);
                if let Some(transform) = snapshot.transform {
                    let _ = world.insert(snapshot.cam_id, transform);
                }
            }
        }

        if effective_play_mode.wants_direct_player_control() {
            let rig = world
                .get::<newengine_sim::CameraRigComp>(cam_id)
                .copied()
                .unwrap_or_default();
            let transform = world.get::<newengine_transform::Transform>(cam_id).copied();
            self.play_session = Some(super::super::controller::PlaySessionSnapshot {
                cam_id,
                rig,
                transform,
            });
        }

        if self.last_play_mode.is_runtime() && !effective_play_mode.is_runtime() {
            if let Some(snapshot) = self.runtime_session.take() {
                restore_runtime_world_snapshot(world, snapshot);
            }
        }
        self.last_play_mode = effective_play_mode;
    }

    fn apply_runtime_input(
        &mut self,
        world: &mut newengine_ecs::World,
        input: &ViewportInputSnap,
        effective_play_mode: EditorPlayMode,
        service_config: CameraRuntimeServiceConfig,
    ) {
        let Some(player) = first_player(world) else {
            return;
        };
        if effective_play_mode.wants_direct_player_control() {
            CameraRuntimeService::apply_player_input(
                world,
                player,
                input.move_mask,
                Vec2::new(-input.dx_px, -input.dy_px),
                input.active,
                service_config.sprint_multiplier,
            );
        } else {
            CameraRuntimeService::clear_player_input(world, player);
        }
    }
}

fn camera_runtime_service_config(world: &newengine_ecs::World) -> CameraRuntimeServiceConfig {
    let mut config = CameraRuntimeServiceConfig::default();
    if let Some(rules) = world.resource::<FpsDemoRules>() {
        config.first_person_eye_height = rules.player.camera_eye_height;
        config.sprint_multiplier = rules.player.sprint_multiplier;
    }
    config
}

fn camera_nav_input(input: &ViewportInputSnap, play_mode: EditorPlayMode) -> CameraNavInput {
    let mut nav_input = CameraNavInput {
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
    };
    if play_mode.wants_direct_player_control() {
        nav_input.wheel_y = 0.0;
        nav_input.pan_drag = false;
    }
    nav_input
}
