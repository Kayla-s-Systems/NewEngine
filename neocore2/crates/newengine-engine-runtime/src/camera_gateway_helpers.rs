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
                .get::<crate::gameplay::PlayerGroundState>(player)
                .is_some_and(|ground| ground.grounded && ground.walkable);
            let weapon_state = world
                .get::<crate::gameplay::PlayerWeaponState>(player)
                .copied();
            let weapon_tuning = world
                .get::<crate::gameplay::HitscanWeaponTuning>(player)
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

            if let Some(anchor) = world
                .get::<crate::gameplay::PlayerFirstPersonCameraAnchor>(player)
                .copied()
                .filter(|anchor| anchor.eye_center_ws.is_finite())
            {
                config.first_person_anchor_ws = Some(anchor.eye_center_ws);
                config.first_person_forward_clearance = if anchor.forward_clearance.is_finite() {
                    anchor.forward_clearance.clamp(0.0, 0.08)
                } else {
                    0.045
                };
            }
            if let Some(render_pose) = world
                .get::<crate::gameplay::PlayerRenderPose>(player)
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
                .get::<crate::gameplay::PlayerRenderPose>(player)
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
            .get::<crate::gameplay::PlayerModelBinding>(player)
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

struct CameraTraceSink {
    writer: BufWriter<File>,
    rows: u64,
}

static CAMERA_TRACE_SINK: OnceLock<Option<StdMutex<CameraTraceSink>>> = OnceLock::new();

fn camera_trace_sink() -> Option<&'static StdMutex<CameraTraceSink>> {
    CAMERA_TRACE_SINK
        .get_or_init(|| {
            let path = crate::env_config::var_os("NEWENGINE_CAMERA_TRACE_FILE")?;
            let file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path)
                .ok()?;
            let mut writer = BufWriter::new(file);
            let _ = writeln!(
                writer,
                "frame,dt,view,raw_dx,raw_dy,routed_dx,routed_dy,sim_x,sim_y,sim_z,render_x,render_y,render_z,fixed_alpha,fixed_tick,runner,yaw,pitch,anchor_x,anchor_y,anchor_z,pivot_x,pivot_y,pivot_z,desired_x,desired_y,desired_z,collision_target,collision_current,rig_x,rig_y,rig_z,pre_x,pre_y,pre_z,final_x,final_y,final_z,frame_blend,frame_blend_alpha,spheres,aabbs,meshes,cached_meshes,bvh_builds,ctrl_z_start,ctrl_z_after_possess,ctrl_z_before_sync,ctrl_z_after_sync,ctrl_z_after_nav,zoom_z"
            );
            Some(StdMutex::new(CameraTraceSink { writer, rows: 0 }))
        })
        .as_ref()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn trace_gameplay_camera_frame(
    frame_index: u64,
    dt: f32,
    input: &CameraGatewayInput,
    routed: Option<RoutedPlayerInput>,
    active_view: CameraViewMode,
    world: &World,
    player: Option<EntityId>,
    camera: EntityId,
    pre_manager_frame: CameraFrame,
    final_frame: CameraFrame,
    report: Option<&CameraRuntimeOverlayReport>,
    controller_z_phases: [f32; 5],
) {
    let Some(sink) = camera_trace_sink() else {
        return;
    };
    let Ok(mut sink) = sink.lock() else {
        return;
    };

    let nan = f32::NAN;
    let (sim, render, fixed_alpha, fixed_tick) = if let Some(player) = player {
        let sim = newengine_transform::read_entity_world_pose_local_chain(world, player)
            .map(|pose| pose.0)
            .unwrap_or(Vec3::splat(nan));
        let render_pose = world
            .get::<crate::gameplay::PlayerRenderPose>(player)
            .copied();
        let render = render_pose
            .map(|pose| pose.position)
            .unwrap_or(Vec3::splat(nan));
        let alpha = render_pose.map(|pose| pose.fixed_alpha).unwrap_or(nan);
        let tick = render_pose.map(|pose| pose.source_fixed_tick).unwrap_or(0);
        (sim, render, alpha, tick)
    } else {
        (Vec3::splat(nan), Vec3::splat(nan), nan, 0)
    };

    let telemetry = CameraRuntimeService::gameplay_camera_telemetry(world, camera);
    let (
        runner,
        yaw,
        pitch,
        anchor,
        pivot,
        desired,
        collision_target,
        collision_current,
        rig,
        zoom_z,
    ) = if let Some(t) = telemetry {
        (
            format!("{:?}", t.runner),
            t.orbit_yaw,
            t.orbit_pitch,
            t.anchor_ws,
            t.pivot_ws,
            t.desired_camera_ws,
            t.collision_target_distance,
            t.collision_distance,
            t.rig_position_ws,
            t.zoom_z,
        )
    } else {
        (
            "None".to_owned(),
            nan,
            nan,
            Vec3::splat(nan),
            Vec3::splat(nan),
            Vec3::splat(nan),
            nan,
            nan,
            Vec3::splat(nan),
            nan,
        )
    };
    let routed = routed.unwrap_or(RoutedPlayerInput {
        move_mask: 0,
        look_delta: Vec2::ZERO,
        look_active: false,
    });
    let collision = world
        .resource::<CameraSpringArmCollisionWorld>()
        .map(|world| world.telemetry())
        .unwrap_or_default();
    let (frame_blend, frame_blend_alpha) = report
        .map(|report| (report.frame_blend_active, report.frame_blend_alpha))
        .unwrap_or((false, 0.0));

    let _ = writeln!(
        sink.writer,
        "{},{:.9},{:?},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{:.7},{:.7},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{:.6},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
        frame_index,
        dt,
        active_view,
        input.dx_px,
        input.dy_px,
        routed.look_delta.x,
        routed.look_delta.y,
        sim.x,
        sim.y,
        sim.z,
        render.x,
        render.y,
        render.z,
        fixed_alpha,
        fixed_tick,
        runner,
        yaw,
        pitch,
        anchor.x,
        anchor.y,
        anchor.z,
        pivot.x,
        pivot.y,
        pivot.z,
        desired.x,
        desired.y,
        desired.z,
        collision_target,
        collision_current,
        rig.x,
        rig.y,
        rig.z,
        pre_manager_frame.rig.position.x,
        pre_manager_frame.rig.position.y,
        pre_manager_frame.rig.position.z,
        final_frame.rig.position.x,
        final_frame.rig.position.y,
        final_frame.rig.position.z,
        frame_blend,
        frame_blend_alpha,
        collision.sphere_count,
        collision.aabb_count,
        collision.mesh_count,
        collision.cached_mesh_count,
        collision.accel_builds_this_refresh,
        controller_z_phases[0],
        controller_z_phases[1],
        controller_z_phases[2],
        controller_z_phases[3],
        controller_z_phases[4],
        zoom_z,
    );
    sink.rows = sink.rows.saturating_add(1);
    if sink.rows % 30 == 0 {
        let _ = sink.writer.flush();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RoutedPlayerInput {
    pub(super) move_mask: u64,
    pub(super) look_delta: Vec2,
    pub(super) look_active: bool,
}

#[inline]
pub(super) fn route_player_input_channels(
    input: &CameraGatewayInput,
    gameplay_capture: newengine_input_capture_api::GameplayInputCapture,
) -> RoutedPlayerInput {
    let movement_blocked = input.gameplay_movement_gated || gameplay_capture.block_player_movement;
    let look_blocked = input.camera_navigation_gated || gameplay_capture.block_camera_navigation;
    let dx_px = if input.dx_px.is_finite() {
        input.dx_px.clamp(-120.0, 120.0)
    } else {
        0.0
    };
    let dy_px = if input.dy_px.is_finite() {
        input.dy_px.clamp(-120.0, 120.0)
    } else {
        0.0
    };
    let raw_look_active = dx_px.abs() > f32::EPSILON || dy_px.abs() > f32::EPSILON;
    RoutedPlayerInput {
        move_mask: if movement_blocked { 0 } else { input.move_mask },
        look_delta: if look_blocked {
            Vec2::ZERO
        } else {
            // Captured-cursor backends can occasionally report warp/recenter spikes. Gameplay
            // look is render-cadence direct, especially Orbit, so bound the packet before it can
            // become a multi-radian angular jump. Generic nav already applies the same policy.
            Vec2::new(-dx_px, -dy_px)
        },
        // A real mouse packet is sufficient evidence that gameplay look is active.
        // The legacy `active` bit comes from viewport/UI routing and can legitimately lag
        // one frame behind raw DeviceEvent motion, which previously dropped pure pitch input.
        look_active: !look_blocked && (input.active || raw_look_active),
    }
}

#[inline]
fn aabb_distance_sq_to_point(min: Vec3, max: Vec3, point: Vec3) -> f32 {
    let nearest = point.clamp(min, max);
    (point - nearest).length_squared()
}

/// Projects query-participating gameplay colliders into the camera-runtime neutral
/// spring-arm collision world. This stays backend-neutral and works even when the
/// physics service is between fixed ticks.
pub(super) fn refresh_camera_spring_arm_collision_world(world: &mut World, player: EntityId) {
    let center = newengine_transform::read_entity_world_pose_local_chain(world, player)
        .map(|pose| pose.0)
        .unwrap_or(Vec3::ZERO);
    const RELEVANCE_RADIUS: f32 = 32.0;
    let relevance_sq = RELEVANCE_RADIUS * RELEVANCE_RADIUS;

    let mut collision_world = world
        .remove_resource::<CameraSpringArmCollisionWorld>()
        .unwrap_or_default();
    collision_world.clear();

    for (entity, body) in world.query::<crate::gameplay::PhysicsBodyDesc>() {
        if body.flags.is_trigger || !body.flags.participates_in_queries {
            continue;
        }
        let Some((position, rotation)) =
            newengine_transform::read_entity_world_pose_local_chain(world, entity)
        else {
            continue;
        };
        let world_from_local = Mat4::from_scale_rotation_translation(Vec3::ONE, rotation, position);
        let bounds = body.shape.local_aabb().transformed(world_from_local);
        if !bounds.min.is_finite()
            || !bounds.max.is_finite()
            || aabb_distance_sq_to_point(bounds.min, bounds.max, center) > relevance_sq
        {
            continue;
        }
        collision_world.push_aabb(CameraSpringArmAabbCollider {
            entity,
            min_ws: bounds.min,
            max_ws: bounds.max,
        });
    }

    for (entity, collider) in world.query::<crate::gameplay::StaticMeshCollider>() {
        let Some((position, rotation)) =
            newengine_transform::read_entity_world_pose_local_chain(world, entity)
        else {
            continue;
        };
        let world_from_local = Mat4::from_scale_rotation_translation(Vec3::ONE, rotation, position);
        let bounds = collider.local_bounds.transformed(world_from_local);
        if !bounds.min.is_finite()
            || !bounds.max.is_finite()
            || aabb_distance_sq_to_point(bounds.min, bounds.max, center) > relevance_sq
        {
            continue;
        }
        collision_world.push_mesh(CameraSpringArmMeshCollider {
            entity,
            revision: collider.revision,
            position_ws: position,
            rotation_ws: rotation.normalize_or_identity(),
            min_ls: collider.local_bounds.min,
            max_ls: collider.local_bounds.max,
            vertices: Arc::clone(&collider.vertices),
            triangles: Arc::clone(&collider.triangles),
        });
    }

    world.insert_resource(collision_world);
}

pub(super) fn apply_runtime_input(
    world: &mut World,
    input: CameraGatewayInput,
    effective_play_mode: GameRunMode,
    service_config: CameraRuntimeServiceConfig,
    frame_index: u64,
) {
    let Some(player) = first_player(world) else {
        return;
    };
    let controller_active = effective_play_mode.wants_direct_player_control()
        && is_player_controller_enabled(world, player);
    let gameplay_capture = crate::gameplay::gameplay_input_capture(world);
    let routed = route_player_input_channels(&input, gameplay_capture);
    let command_actions = if controller_active {
        input.gameplay_actions
    } else {
        ActionCommandFrame::default()
    };
    apply_player_command_frame(world, player, frame_index, command_actions);

    if controller_active {
        // Movement and camera look are independent gameplay channels. A dialogue/menu layer
        // may deliberately freeze locomotion while preserving free look; conversely a scripted
        // camera may suppress look without cancelling WASD. Do not collapse either policy into
        // a total player-input gate.
        CameraRuntimeService::apply_player_input(
            world,
            player,
            routed.move_mask,
            routed.look_delta,
            routed.look_active,
            service_config.sprint_multiplier,
            service_config.runner,
            matches!(
                service_config.runner,
                newengine_camera_runtime::GameplayCameraRunnerKind::FirstPerson
                    | newengine_camera_runtime::GameplayCameraRunnerKind::ThirdPersonAim
            ),
        );
        emit_player_event(
            world,
            player,
            PlayerEventKind::InputApplied,
            "local input sampled",
        );
    } else {
        CameraRuntimeService::clear_player_input(world, player);
    }
}

#[inline]
pub(super) fn camera_nav_input(
    input: CameraGatewayInput,
    play_mode: GameRunMode,
) -> CameraNavInput {
    let mut nav_input = CameraNavInput {
        dx_px: finite_or_zero(input.dx_px).clamp(-240.0, 240.0),
        dy_px: finite_or_zero(input.dy_px).clamp(-240.0, 240.0),
        wheel_y: finite_or_zero(input.wheel_y).clamp(-12.0, 12.0),
        active: input.active,
        look_drag: input.look_drag,
        pan_drag: input.pan_drag,
        ui_busy: input.ui_busy,
        fly_rmb: input.fly_rmb,
        navigation_gated: input.camera_navigation_gated,
        move_mask: input.move_mask,
        speed_scalar: finite_or_one(input.speed_scalar).clamp(0.05, 20.0),
    };
    if play_mode.wants_direct_player_control() {
        nav_input.wheel_y = 0.0;
        nav_input.pan_drag = false;
    }
    if nav_input.navigation_gated {
        nav_input.gate_navigation();
    }
    nav_input
}

#[inline]
pub(super) fn apply_gameplay_view_lens(
    frame: CameraFrame,
    active_view: CameraViewMode,
    first_person_aiming: bool,
) -> CameraFrame {
    let target_fov_y = match active_view {
        // RMB/ADS narrows the lens while the view-model moves onto the sight line. Hip-fire keeps
        // the wider ~100-degree horizontal presentation used by normal first-person traversal.
        CameraViewMode::FirstPerson if first_person_aiming => 45.0_f32.to_radians(),
        CameraViewMode::FirstPerson => 68.0_f32.to_radians(),
        CameraViewMode::ThirdPersonFollow => 64.0_f32.to_radians(),
        CameraViewMode::ThirdPersonAim => 54.0_f32.to_radians(),
        CameraViewMode::ThirdPersonOrbit => 60.0_f32.to_radians(),
    };
    let Projection::Perspective(mut perspective) = frame.projection else {
        return frame;
    };
    let target_near = if matches!(active_view, CameraViewMode::FirstPerson) {
        // Full-body FPP shares world geometry with the character. A 1 cm near plane exposes
        // backfaces inside face/clothing shells; 4.5 cm still preserves close interaction while
        // behaving like a dedicated FPP render contract.
        0.045
    } else {
        perspective.near
    };
    if (perspective.fovy - target_fov_y).abs() <= 1.0e-6
        && (perspective.near - target_near).abs() <= 1.0e-6
    {
        return frame;
    }
    perspective.fovy = target_fov_y;
    perspective.near = target_near;
    CameraFrame::build(
        frame.channel,
        frame.rig,
        Projection::Perspective(perspective),
        frame.viewport,
        frame.jitter_px,
    )
}

#[inline]
pub(super) fn view_postfx_from_camera_snapshot(
    snapshot: CameraFrameSnapshot,
) -> ViewPostFxFrameParams {
    let postfx = snapshot.postfx;
    ViewPostFxFrameParams {
        dof: ViewDepthOfFieldFrameParams {
            near_start: postfx.dof.near_start,
            near_end: postfx.dof.near_end,
            far_start: postfx.dof.far_start,
            far_end: postfx.dof.far_end,
            blend_level: postfx.dof.blend_level,
            high_quality: postfx.dof.high_quality,
        },
        motion_blur: ViewMotionBlurFrameParams {
            strength: postfx.motion_blur.strength,
            decay_rate: postfx.motion_blur.decay_rate,
        },
        shake_amplitude: postfx.shake_amplitude,
        exposure_bias: postfx.exposure_bias,
        jitter_px: postfx.jitter_px,
    }
}

#[inline]
pub(super) fn camera_report_snapshot(report: CameraRuntimeReport) -> CameraRuntimeOverlayReport {
    CameraRuntimeOverlayReport {
        active_director: format!("{:?}", report.active_director),
        active_mode: format!("{:?}", report.active_mode),
        active_view_mode: format!("{:?}", report.view_mode),
        target_entity: report.target_entity,
        transition: CameraTransitionOverlayReport {
            phase: match report.transition.phase {
                RuntimeCameraTransitionPhase::Idle => CameraTransitionPhase::Idle,
                RuntimeCameraTransitionPhase::Pending => CameraTransitionPhase::Pending,
                RuntimeCameraTransitionPhase::Blending => CameraTransitionPhase::Blending,
            },
            elapsed_sec: report.transition.elapsed_sec,
        },
        input_context: format!("{:?}", report.input_context),
        gate_blocked: report.gate_blocked,
        frame_blend_active: report.frame_blend_active,
        frame_blend_alpha: report.frame_blend_alpha,
        dominant_director: report.dominant_director.map(|it| format!("{:?}", it)),
        rendered_director_count: report.rendered_director_count,
        director_lock_input: report.director_lock_input,
        pending_event_count: report.pending_event_count,
    }
}

#[inline]
fn mat4_from_cols(cols: [[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array(&[
        cols[0][0], cols[0][1], cols[0][2], cols[0][3], cols[1][0], cols[1][1], cols[1][2],
        cols[1][3], cols[2][0], cols[2][1], cols[2][2], cols[2][3], cols[3][0], cols[3][1],
        cols[3][2], cols[3][3],
    ])
}

#[inline]
fn arr_vec3(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[1], v[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_gate_preserves_free_look_xy() {
        let input = CameraGatewayInput {
            dx_px: 12.0,
            dy_px: -7.5,
            active: true,
            gameplay_movement_gated: true,
            move_mask: 0x0f,
            ..CameraGatewayInput::default()
        };
        let routed = route_player_input_channels(
            &input,
            newengine_input_capture_api::GameplayInputCapture::none(),
        );
        assert_eq!(routed.move_mask, 0);
        assert_eq!(routed.look_delta, Vec2::new(-12.0, 7.5));
        assert!(routed.look_active);
    }

    #[test]
    fn pure_vertical_mouse_packet_activates_look_even_when_legacy_active_is_stale() {
        let input = CameraGatewayInput {
            dx_px: 0.0,
            dy_px: 9.0,
            active: false,
            move_mask: 0,
            ..CameraGatewayInput::default()
        };
        let routed = route_player_input_channels(
            &input,
            newengine_input_capture_api::GameplayInputCapture::none(),
        );
        assert_eq!(routed.look_delta, Vec2::new(0.0, -9.0));
        assert!(routed.look_active);
    }

    #[test]
    fn gameplay_camera_input_clamps_captured_cursor_warp_spikes() {
        let input = CameraGatewayInput {
            dx_px: 4200.0,
            dy_px: -3600.0,
            active: true,
            ..CameraGatewayInput::default()
        };
        let routed = route_player_input_channels(
            &input,
            newengine_input_capture_api::GameplayInputCapture::none(),
        );
        assert_eq!(routed.look_delta, Vec2::new(-120.0, 120.0));
        assert!(routed.look_active);
    }

    #[test]
    fn camera_gate_blocks_look_without_cancelling_movement() {
        let input = CameraGatewayInput {
            dx_px: 12.0,
            dy_px: -7.5,
            active: true,
            camera_navigation_gated: true,
            move_mask: 0x03,
            ..CameraGatewayInput::default()
        };
        let routed = route_player_input_channels(
            &input,
            newengine_input_capture_api::GameplayInputCapture::none(),
        );
        assert_eq!(routed.move_mask, 0x03);
        assert_eq!(routed.look_delta, Vec2::ZERO);
        assert!(!routed.look_active);
    }
    #[test]
    fn orbit_config_uses_bound_avatar_visual_center_not_capsule_top() {
        use newengine_transform::{set_parent, Transform};

        let mut world = World::new();
        let player = world.spawn();
        let visual_root = world.spawn();
        let _ = world.insert(player, crate::gameplay::PlayerActor);
        let _ = world.insert(player, crate::gameplay::PlayerController::local_input());
        let _ = world.insert(player, CharacterBody::default());
        let _ = world.insert(player, Transform::default());
        let _ = world.insert(
            visual_root,
            Transform {
                // Model root sits on the capsule ground plane plus an authored local offset.
                position: Vec3::new(0.20, -0.80, -0.15),
                rotation: newengine_math::Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        let _ = set_parent(&mut world, visual_root, Some(player));
        let _ = world.insert(
            player,
            crate::gameplay::PlayerModelBinding {
                visual_root: Some(visual_root),
                target_height: 1.80,
                ..Default::default()
            },
        );

        let config = camera_runtime_service_config(&world, CameraViewMode::ThirdPersonOrbit);
        let expected = Vec3::new(0.20, 0.10, -0.15);
        assert!((config.third_person_orbit_pivot_offset_ls - expected).length() < 1.0e-5);
    }
    #[test]
    fn third_person_config_carries_interpolated_player_render_pose() {
        use newengine_transform::Transform;

        let mut world = World::new();
        let player = world.spawn();
        let _ = world.insert(player, crate::gameplay::PlayerActor);
        let _ = world.insert(player, crate::gameplay::PlayerController::local_input());
        let _ = world.insert(player, CharacterBody::default());
        let _ = world.insert(
            player,
            Transform {
                position: Vec3::new(8.0, 0.0, 0.0),
                rotation: newengine_math::Quat::from_rotation_y(1.0),
                scale: Vec3::ONE,
            },
        );
        let render_position = Vec3::new(3.25, 0.0, -1.5);
        let render_rotation = newengine_math::Quat::from_rotation_y(0.35);
        let _ = world.insert(
            player,
            crate::gameplay::PlayerRenderPose {
                position: render_position,
                rotation: render_rotation,
                simulation_position: Vec3::new(8.0, 0.0, 0.0),
                simulation_rotation: newengine_math::Quat::from_rotation_y(1.0),
                fixed_alpha: 0.4,
                source_fixed_tick: 42,
            },
        );

        let config = camera_runtime_service_config(&world, CameraViewMode::ThirdPersonOrbit);
        assert_eq!(
            config.third_person_render_position_ws,
            Some(render_position)
        );
        let configured_rotation = config.third_person_render_rotation_ws.unwrap();
        assert!(configured_rotation.dot(render_rotation).abs() > 0.999999);
    }
}
