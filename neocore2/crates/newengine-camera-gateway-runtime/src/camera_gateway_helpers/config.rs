use super::*;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::{Mutex as StdMutex, OnceLock};

impl EngineViewFrame {
    #[inline]
    pub(super) fn from_camera_snapshot(snapshot: CameraFrameSnapshot) -> Self {
        Self {
            view: mat4_from_cols(snapshot.view_cols),
            projection: mat4_from_cols(snapshot.projection_cols),
            view_projection: mat4_from_cols(snapshot.view_projection_cols),
            inverse_view: mat4_from_cols(snapshot.inverse_view_cols),
            position_ws: arr_vec3(snapshot.position_ws),
            position_ws_f64: snapshot.position_ws_f64,
            world_origin_ws_f64: snapshot.world_origin_ws_f64,
            position_origin_relative_ws: arr_vec3(snapshot.position_origin_relative_ws),
            forward_ws: arr_vec3(snapshot.forward_ws),
            viewport_width: snapshot.viewport.width,
            viewport_height: snapshot.viewport.height,
            aspect: snapshot.viewport.aspect,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraRuntimeOverlayReport {
    pub active_director: String,
    pub active_mode: String,
    pub active_view_mode: String,
    pub target_entity: Option<EntityId>,
    pub transition: CameraTransitionOverlayReport,
    pub input_context: String,
    pub gate_blocked: bool,
    pub frame_blend_active: bool,
    pub frame_blend_alpha: f32,
    pub dominant_director: Option<String>,
    pub rendered_director_count: usize,
    pub director_lock_input: bool,
    pub pending_event_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraTransitionPhase {
    Idle,
    Pending,
    Blending,
}

#[derive(Clone, Debug)]
pub struct CameraTransitionOverlayReport {
    pub phase: CameraTransitionPhase,
    pub elapsed_sec: f32,
}

#[inline]
pub fn apply_view_postfx(
    mut params: PostFxFrameParams,
    view: ViewPostFxFrameParams,
) -> PostFxFrameParams {
    params.display.exposure *= 2.0f32.powf(view.exposure_bias);
    params.view = view;
    params
}

#[inline]
pub(super) fn sanitize_camera_dt(dt: f32) -> f32 {
    if !dt.is_finite() || dt <= 0.0 {
        return 0.0;
    }
    // Camera navigation must not integrate a whole stall in one frame: render/asset
    // hitch recovery should not teleport the view or explode springs.
    dt.min(1.0 / 20.0)
}

#[inline]
fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

#[inline]
fn finite_or_one(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        1.0
    }
}

#[inline]
pub(super) fn camera_runtime_service_config(
    world: &World,
    active_view: CameraViewMode,
) -> CameraRuntimeServiceConfig {
    let mut config = CameraRuntimeServiceConfig::default();
    if let Some(player) = first_player(world) {
        if let Some(profile) = world
            .get::<newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile>(player)
            .copied()
            .map(newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile::sanitized)
        {
            config.first_person_forward_clearance = profile.first_person_forward_clearance;
            config.first_person_body_yaw_limit_radians =
                profile.first_person_body_yaw_limit_radians;
            config.first_person_fov_y_radians = profile.first_person_fov_y_radians;
            config.first_person_ads_fov_y_radians = profile.first_person_ads_fov_y_radians;
            config.first_person_near = profile.first_person_near;
            config.first_person_collision_enabled = profile.first_person_collision_enabled;
            config.first_person_collision_probe_radius =
                profile.first_person_collision_probe_radius;
            config.first_person_collision_padding = profile.first_person_collision_padding;
            config.first_person_grounded_eye_deadband_m =
                profile.first_person_grounded_eye_deadband_m;
            config.first_person_grounded_eye_time_constant_seconds =
                profile.first_person_grounded_eye_time_constant_seconds;
            config.first_person_camera_recoil_share = profile.first_person_camera_recoil_share;
            config.first_person_aim_response_hz = profile.first_person_aim_response_hz;
            config.near_clip_enabled = profile.near_clip_enabled;
            config.near_clip_first_person_max_distance =
                profile.near_clip_first_person_max_distance;
            config.near_clip_third_person_min_distance =
                profile.near_clip_third_person_min_distance;
            config.near_clip_third_person_max_distance =
                profile.near_clip_third_person_max_distance;
            config.near_clip_pull_in_distance = profile.near_clip_pull_in_distance;
            config.near_clip_probe_radius = profile.near_clip_probe_radius;
            config.near_clip_release_time_seconds = profile.near_clip_release_time_seconds;
            config.near_clip_hysteresis_m = profile.near_clip_hysteresis_m;
            config.third_person_follow_fov_y_radians = profile.third_person_follow_fov_y_radians;
            config.third_person_follow_offset_ls = profile.third_person_follow_offset_ls;
            config.third_person_follow_focus_offset_ls =
                profile.third_person_follow_focus_offset_ls;
            config.third_person_follow_smooth_time = profile.third_person_follow_smooth_time;
            config.third_person_follow_max_speed = profile.third_person_follow_max_speed;
            config.third_person_follow_zoom_min = profile.third_person_follow_zoom_min;
            config.third_person_follow_zoom_max = profile.third_person_follow_zoom_max;
            config.third_person_aim_fov_y_radians = profile.third_person_aim_fov_y_radians;
            config.third_person_aim_offset_ls = profile.third_person_aim_offset_ls;
            config.third_person_aim_focus_offset_ls = profile.third_person_aim_focus_offset_ls;
            config.third_person_aim_smooth_time = profile.third_person_aim_smooth_time;
            config.third_person_aim_max_speed = profile.third_person_aim_max_speed;
            config.third_person_aim_zoom_min = profile.third_person_aim_zoom_min;
            config.third_person_aim_zoom_max = profile.third_person_aim_zoom_max;
            config.third_person_orbit_fov_y_radians = profile.third_person_orbit_fov_y_radians;
            config.third_person_orbit_offset_ls = profile.third_person_orbit_offset_ls;
            config.third_person_orbit_focus_offset_ls = profile.third_person_orbit_focus_offset_ls;
            config.third_person_orbit_smooth_time = profile.third_person_orbit_smooth_time;
            config.third_person_orbit_max_speed = profile.third_person_orbit_max_speed;
            config.third_person_orbit_zoom_min = profile.third_person_orbit_zoom_min;
            config.third_person_orbit_zoom_max = profile.third_person_orbit_zoom_max;
            config.third_person_orbit_look_sensitivity_radians_per_pixel =
                profile.third_person_orbit_look_sensitivity_radians_per_pixel;
            config.third_person_orbit_pitch_min_radians =
                profile.third_person_orbit_pitch_min_radians;
            config.third_person_orbit_pitch_max_radians =
                profile.third_person_orbit_pitch_max_radians;
            config.third_person_collision_enabled = profile.third_person_collision_enabled;
            config.third_person_collision_probe_radius =
                profile.third_person_collision_probe_radius;
            config.third_person_collision_padding = profile.third_person_collision_padding;
            config.third_person_collision_min_distance =
                profile.third_person_collision_min_distance;
            config.third_person_collision_release_frequency_hz =
                profile.third_person_collision_release_frequency_hz;
            config.third_person_collision_release_damping_ratio =
                profile.third_person_collision_release_damping_ratio;
            config.third_person_collision_distance_hysteresis =
                profile.third_person_collision_distance_hysteresis;
            config.third_person_look_at_collision_blend =
                profile.third_person_look_at_collision_blend;
            config.third_person_look_at_response_hz = profile.third_person_look_at_response_hz;
            config.third_person_look_at_max_error_fov_fraction =
                profile.third_person_look_at_max_error_fov_fraction;
            config.third_person_catch_up_enabled = profile.third_person_catch_up_enabled;
            config.third_person_catch_up_frequency_hz = profile.third_person_catch_up_frequency_hz;
            config.third_person_catch_up_damping_ratio =
                profile.third_person_catch_up_damping_ratio;
            config.third_person_catch_up_max_distance_m =
                profile.third_person_catch_up_max_distance_m;
            config.third_person_catch_up_settle_distance_m =
                profile.third_person_catch_up_settle_distance_m;
            config.zoom_wheel_exponent_per_step = profile.zoom_wheel_exponent_per_step;
            config.orbit_drag_zoom_exponent_per_pixel = profile.orbit_drag_zoom_exponent_per_pixel;
            config.zoom_smooth_time_seconds = profile.zoom_smooth_time_seconds;
        }
        if let Some(body) = world.get::<CharacterBody>(player) {
            let body = body.sanitized();
            config.first_person_eye_height = world
                .get::<PlayerStanceState>(player)
                .map(|state| state.current_eye_height)
                .unwrap_or(body.standing_eye_height);
        }

        if matches!(active_view, CameraViewMode::FirstPerson) {
            let velocity = world
                .get::<newengine_sim::Velocity>(player)
                .copied()
                .unwrap_or_default()
                .0;
            let horizontal_speed = Vec3::new(velocity.x, 0.0, velocity.z).length();
            let grounded = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerGroundState>(player)
                .is_some_and(|ground| ground.grounded && ground.walkable);
            let weapon_state = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerWeaponState>(player)
                .copied();
            let weapon_tuning = world
                .get::<newengine_gameplay_world_runtime::gameplay::HitscanWeaponTuning>(player)
                .copied()
                .map(|tuning| tuning.sanitized());
            let mut presentation = newengine_camera_runtime::FirstPersonPresentationInput {
                grounded,
                horizontal_speed,
                aiming: active_weapon_aim_intent(world, player),
                shot_sequence: weapon_state.map(|state| state.shot_sequence).unwrap_or(0),
                ..Default::default()
            };
            if let Some(tuning) = weapon_tuning {
                presentation.recoil_pitch_radians = tuning.recoil_pitch_radians;
                presentation.recoil_pitch_random_radians = tuning.recoil_pitch_random_radians;
                presentation.recoil_yaw_radians = tuning.recoil_yaw_radians;
                presentation.recoil_yaw_bias_radians = tuning.recoil_yaw_bias_radians;
                presentation.ads_recoil_multiplier = tuning.ads_recoil_multiplier;
                presentation.recoil_recovery_hz = tuning.recoil_recovery_hz;
            }
            config.first_person_presentation = presentation;

            let body = world
                .get::<CharacterBody>(player)
                .copied()
                .unwrap_or_default()
                .sanitized();
            let barrier = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerFirstPersonBodyBarrierProfile>(player)
                .copied()
                .unwrap_or_else(|| {
                    newengine_gameplay_world_runtime::gameplay::PlayerFirstPersonBodyBarrierProfile::from_body(body)
                })
                .sanitized(body);
            config.first_person_body_barrier =
                newengine_camera_runtime::FirstPersonBodyBarrierInput {
                    enabled: barrier.enabled,
                    head_center_offset_ls: barrier.head_center_offset_ls,
                    head_radius: barrier.head_radius,
                    neck_top_offset_ls: barrier.neck_top_offset_ls,
                    neck_bottom_offset_ls: barrier.neck_bottom_offset_ls,
                    neck_radius: barrier.neck_radius,
                    chest_top_offset_ls: barrier.chest_top_offset_ls,
                    chest_bottom_offset_ls: barrier.chest_bottom_offset_ls,
                    chest_radius: barrier.chest_radius,
                    surface_padding: barrier.surface_padding,
                    downward_pitch_limit_radians: barrier.downward_pitch_limit_radians,
                };
            if let Some(profile) = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile>(player)
                .copied()
                .map(newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile::sanitized)
            {
                config
                    .first_person_body_barrier
                    .downward_pitch_limit_radians = profile.first_person_down_pitch_limit_radians;
            }

            if let Some(anchor) = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerFirstPersonCameraAnchor>(
                    player,
                )
                .copied()
                .filter(|anchor| anchor.eye_center_ws.is_finite())
            {
                config.first_person_anchor_ws = Some(anchor.eye_center_ws);
                config.first_person_ads_anchor_ws = anchor
                    .ads_camera_position_ws
                    .filter(|position| position.is_finite());
                // Avatar providers publish the resolved anchor; project camera authoring owns
                // the clearance value. Accept provider clearance only for legacy players that
                // have no PlayerCameraProfile component.
                if world
                    .get::<newengine_gameplay_world_runtime::gameplay::PlayerCameraProfile>(player)
                    .is_none()
                {
                    config.first_person_forward_clearance = if anchor.forward_clearance.is_finite()
                    {
                        anchor.forward_clearance.clamp(0.0, 0.25)
                    } else {
                        config.first_person_forward_clearance
                    };
                }
            }
            if let Some(render_pose) = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerRenderPose>(player)
                .copied()
                .filter(|pose| pose.rotation.is_finite())
            {
                config.first_person_body_rotation_ws =
                    Some(render_pose.rotation.normalize_or_identity());
            }
        }

        // PlayerActor is the physics-capsule center, not the avatar's feet. A bound runtime
        // model has its own child visual_root (normally lowered by the capsule ground offset),
        // so deriving Orbit from CharacterBody::visual_half_height aims too high. Resolve the
        // actual visual center and convert it back into PlayerActor-local space. Fallback capsule
        // visuals are centered directly on PlayerActor, hence Vec3::ZERO is the correct fallback.
        if !matches!(active_view, CameraViewMode::FirstPerson) {
            if let Some(render_pose) = world
                .get::<newengine_gameplay_world_runtime::gameplay::PlayerRenderPose>(player)
                .copied()
            {
                if render_pose.position.is_finite() && render_pose.rotation.is_finite() {
                    config.third_person_render_position_ws = Some(render_pose.position);
                    config.third_person_render_rotation_ws =
                        Some(render_pose.rotation.normalize_or_identity());
                }
            }
        }

        config.third_person_orbit_pivot_offset_ls = world
            .get::<newengine_gameplay_world_runtime::gameplay::PlayerModelBinding>(player)
            .and_then(|binding| {
                let visual_root = binding.visual_root.filter(|entity| world.exists(*entity))?;
                // visual_root is parented directly under PlayerActor. Its local Transform is the
                // authored model alignment and is stable; deriving the same offset from two
                // separately resolved world-space chains mixes simulation/presentation cadence
                // and lets tiny propagation differences move the Orbit pivot every render frame.
                let visual_local = world.get::<Transform>(visual_root).copied()?;
                let target_height = if binding.target_height.is_finite() {
                    binding.target_height.clamp(0.25, 3.0)
                } else {
                    1.80
                };
                Some(
                    visual_local.position
                        + visual_local.rotation.normalize_or_identity()
                            * Vec3::Y
                            * (target_height * 0.5),
                )
            })
            .filter(|offset| offset.is_finite())
            .unwrap_or(Vec3::ZERO);
        if let Some(motion) = world.get::<CharacterMotionTuning>(player) {
            config.sprint_multiplier = motion.sanitized().sprint_multiplier;
        }
    }
    config.runner = match active_view {
        CameraViewMode::FirstPerson => {
            newengine_camera_runtime::GameplayCameraRunnerKind::FirstPerson
        }
        CameraViewMode::ThirdPersonFollow => {
            newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonFollow
        }
        CameraViewMode::ThirdPersonAim => {
            newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonAim
        }
        CameraViewMode::ThirdPersonOrbit => {
            newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonOrbit
        }
    };
    config
}

#[inline]
pub(super) fn follow_controller_offset_z(world: &World, camera: EntityId) -> f32 {
    world
        .get::<newengine_sim::FollowTargetCameraController>(camera)
        .map(|controller| controller.offset_ls.z)
        .filter(|value| value.is_finite())
        .unwrap_or(f32::NAN)
}
