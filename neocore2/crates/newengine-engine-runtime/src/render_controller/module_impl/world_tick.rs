use newengine_core::physics::PhysicsApiRef;
use newengine_core::render::RenderApi;
use newengine_scene::Scene;

use crate::engine_bounds::EngineBoundsSnap;
use crate::scene_bridge::EngineViewInput;
use crate::gameplay::{run_schedule, GameRunMode};

use super::frame_types::WorldFrameState;
use super::input::ViewportInputSnap;
use super::super::controller::RuntimeRenderController;
use super::{readiness, scene};

impl RuntimeRenderController {
    pub(super) fn tick_world_for_render(
        &mut self,
        r: &mut dyn RenderApi,
        physics_api: Option<&PhysicsApiRef>,
        job_system: Option<&newengine_core::JobSystemHandle>,
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

            if effective_play_mode.is_runtime() {
                let mats_lock = scene_bridge.materials();
                let mats = mats_lock.read();
                crate::scene_bridge::tick_game_ready_streaming_terrain(world, &mats, job_system);
            }
            crate::scene_bridge::tick_game_ready_sky_cycle(world, dt);

            if effective_play_mode.runs_physics() && !pause_world {
                world.insert_resource(crate::gameplay::PhysicsRuntimeFrameIndex(self.frame.frame_index));
                run_schedule(&mut self.frame.sim_schedule, world, dt, physics_api);
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
            self.bridges.scene.activate_game_ready_play_now();
        }

        WorldFrameState { view_frame }
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
