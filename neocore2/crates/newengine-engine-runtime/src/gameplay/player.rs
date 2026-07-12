use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::{move_mask as input_move, GameplayActionFrame};
use newengine_math::{Quat, Vec2, Vec3};
use newengine_primitives::{builtins as prim_builtins, Primitive};
use newengine_scene::components::Name;
use newengine_sim::{
    CameraRigComp, CharacterMotor, FollowTargetCameraController, FollowTargetCameraMotor,
    MotorInput, Velocity,
};
use newengine_transform::{set_parent, Transform};

use super::listeners::emit_player_event;
use super::{
    ensure_inventory_hud_state, give_default_fps_loadout, CollisionShapeDesc, DisplayMode,
    DisplayVisibility, FpsDemoRules, FpsPlayerTuning, GameplayActor, Health, HitscanWeaponTuning,
    PhysicsBodyDesc, PhysicsSurface, PlayerActor, PlayerCommandFrame, PlayerController,
    PlayerEventKind, PlayerGroundState, PlayerInteractionTuning, PlayerLocomotionState,
    PlayerModelBinding, PlayerStanceKind, PlayerStanceState, PlayerViewVisibility,
    PlayerVisualKind, PlayerVisualPart, PlayerWeaponState,
};

#[inline]
pub fn ensure_physics_body(world: &mut World, entity: EntityId, body: PhysicsBodyDesc) {
    let _ = world.insert(entity, body);
    let _ = world.insert(entity, body.to_bounds());
    if world.get::<PhysicsSurface>(entity).is_none() {
        let _ = world.insert(entity, PhysicsSurface::default());
    }
}

#[inline]
pub fn remove_physics_body(world: &mut World, entity: EntityId) {
    let _ = world.remove::<PhysicsBodyDesc>(entity);
}

#[inline]
pub fn spawn_default_player(
    world: &mut World,
    root: Option<EntityId>,
    name: impl Into<String>,
    position: Vec3,
) -> EntityId {
    spawn_default_player_with_tuning(world, root, name, position, FpsPlayerTuning::default())
}

#[inline]
pub fn spawn_default_player_with_tuning(
    world: &mut World,
    root: Option<EntityId>,
    name: impl Into<String>,
    position: Vec3,
    tuning: FpsPlayerTuning,
) -> EntityId {
    spawn_player_controller_with_tuning(world, root, name, position, tuning, true)
}

/// Spawns a player as a normal ECS entity composition:
///
/// - root entity: identity/controller/physics/motor components;
/// - optional fallback visual child: renderable capsule component;
/// - future or imported models attach as additional visual child entities.
///
/// This keeps local input possession out of entity identity. The player is not a
/// singleton object; it is just the lowest stable entity with `PlayerActor` plus
/// an enabled `PlayerController`.
pub fn spawn_player_controller_with_tuning(
    world: &mut World,
    root: Option<EntityId>,
    name: impl Into<String>,
    position: Vec3,
    tuning: FpsPlayerTuning,
    spawn_fallback_visual: bool,
) -> EntityId {
    let tuning = tuning.sanitized();
    let name = name.into();
    let e = world.spawn();

    let _ = world.insert(e, Name(name.clone()));
    let _ = world.insert(
        e,
        Transform {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(e, GameplayActor);
    let _ = world.insert(e, PlayerActor);
    let _ = world.insert(e, PlayerController::local_input());
    let _ = world.insert(e, PlayerCommandFrame::default());
    let weapon_tuning = HitscanWeaponTuning::default().sanitized();
    let _ = world.insert(e, weapon_tuning);
    let _ = world.insert(e, PlayerWeaponState::loaded(weapon_tuning));
    let _ = world.insert(e, PlayerInteractionTuning::default());
    let _ = world.insert(e, Health::default());
    // Inventory owns equipment selection and reserve ammunition. The direct weapon
    // components above remain a safe fallback if authored loadout installation fails.
    ensure_inventory_hud_state(world);
    let _ = give_default_fps_loadout(world, e);
    let _ = world.insert(e, PlayerGroundState::default());
    let _ = world.insert(e, PlayerLocomotionState::default());
    let _ = world.insert(e, PlayerStanceState::standing(tuning.camera_eye_height));
    let _ = world.insert(e, PlayerModelBinding::default());
    let _ = world.insert(e, CharacterMotor::default());
    let _ = world.insert(e, MotorInput::default());
    let _ = world.insert(e, Velocity(Vec3::ZERO));

    ensure_physics_body(
        world,
        e,
        PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Capsule {
            radius: tuning.body_radius,
            half_height: tuning.body_half_height,
        }),
    );

    if let Some(root) = root.filter(|id| world.exists(*id)) {
        let _ = set_parent(world, e, Some(root));
    }

    if spawn_fallback_visual {
        spawn_fallback_player_visual(world, e, &name, tuning);
    }

    emit_player_event(
        world,
        e,
        PlayerEventKind::Spawned,
        format!("player entity spawned name='{name}'"),
    );

    e
}

fn spawn_fallback_player_visual(
    world: &mut World,
    owner: EntityId,
    owner_name: &str,
    tuning: FpsPlayerTuning,
) -> EntityId {
    let visual = world.spawn();
    let _ = world.insert(visual, Name(format!("{owner_name}/Visual/FallbackCapsule")));
    let _ = world.insert(
        visual,
        Transform {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(
                tuning.visual_radius,
                tuning.visual_half_height,
                tuning.visual_radius,
            ),
        },
    );
    let _ = world.insert(
        visual,
        Primitive {
            id: prim_builtins::ID_CAPSULE,
            color: [0.30, 0.72, 0.98, 1.0],
        },
    );
    let _ = world.insert(visual, GameplayActor);
    let _ = world.insert(
        visual,
        PlayerVisualPart {
            owner,
            part_index: 0,
            kind: PlayerVisualKind::FallbackCapsule,
            material_slot: "fallback_capsule".to_owned(),
        },
    );
    let _ = world.insert(visual, PlayerViewVisibility::fallback_capsule_default());
    let _ = set_parent(world, visual, Some(owner));
    visual
}

#[inline]
pub fn first_player(world: &World) -> Option<EntityId> {
    let mut best: Option<EntityId> = None;
    for (id, _) in world.query::<PlayerActor>() {
        match best {
            Some(cur) if cur.stable_u64() <= id.stable_u64() => {}
            _ => best = Some(id),
        }
    }
    best
}

#[inline]
pub fn is_player_controller_enabled(world: &World, player: EntityId) -> bool {
    world
        .get::<PlayerController>(player)
        .map(|controller| controller.enabled)
        .unwrap_or(true)
}

#[inline]
pub fn clear_player_input(world: &mut World, player: EntityId) {
    if let Some(input) = world.get_mut::<MotorInput>(player) {
        *input = MotorInput::default();
    }
}

#[inline]
pub fn apply_player_command_frame(
    world: &mut World,
    player: EntityId,
    source_frame: u64,
    actions: GameplayActionFrame,
) {
    if !world.exists(player) {
        return;
    }
    if let Some(pending) = world.get_mut::<PlayerCommandFrame>(player) {
        pending.source_frame = pending.source_frame.max(source_frame);
        pending.actions.merge_pending(actions);
    } else {
        let _ = world.insert(player, PlayerCommandFrame::new(source_frame, actions));
    }
}

pub fn apply_player_input(
    world: &mut World,
    player: EntityId,
    input_mask: u64,
    look_delta_px: Vec2,
    look_active: bool,
) {
    if !is_player_controller_enabled(world, player) {
        clear_player_input(world, player);
        return;
    }

    let mut axis = Vec3::ZERO;

    if input_mask & input_move::FORWARD != 0 {
        axis.z += 1.0;
    }
    if input_mask & input_move::BACK != 0 {
        axis.z -= 1.0;
    }
    if input_mask & input_move::RIGHT != 0 {
        axis.x += 1.0;
    }
    if input_mask & input_move::LEFT != 0 {
        axis.x -= 1.0;
    }
    if input_mask & input_move::UP != 0 {
        axis.y += 1.0;
    }
    if input_mask & input_move::DOWN != 0 {
        axis.y -= 1.0;
    }

    let sprint_multiplier = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sprint_multiplier)
        .unwrap_or_else(|| FpsPlayerTuning::default().sprint_multiplier);

    let mut applied = false;
    if let Some(input) = world.get_mut::<MotorInput>(player) {
        input.move_axis = axis;
        input.look_delta += look_delta_px;
        input.look_active = look_active;
        input.speed_mul = if input_mask & input_move::SPRINT != 0 {
            sprint_multiplier
        } else {
            1.0
        };
        input.zoom_delta = 0.0;
        applied = true;
    }
    if applied {
        emit_player_event(
            world,
            player,
            PlayerEventKind::InputApplied,
            "local input sampled",
        );
    }
}

/// Applies semantic commands that must affect the next physics step.
pub fn apply_player_fixed_commands(world: &mut World, dt: f32, fixed_tick: u64) {
    let tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_else(|| FpsPlayerTuning::default().sanitized());
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();

    for player in players {
        if !is_player_controller_enabled(world, player) {
            continue;
        }
        let actions = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| commands.actions)
            .unwrap_or_default();
        let stance = world
            .get::<PlayerStanceState>(player)
            .copied()
            .unwrap_or_else(|| PlayerStanceState::standing(tuning.camera_eye_height));

        if actions.crouch_held {
            if stance.current != PlayerStanceKind::Crouched {
                let _ = apply_player_stance_geometry(
                    world,
                    player,
                    PlayerStanceKind::Crouched,
                    tuning,
                    fixed_tick,
                );
            }
            if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
                state.stand_requested = false;
                state.stand_blocked = false;
                state.target_eye_height = tuning.crouched_camera_eye_height;
            }
        } else if stance.current == PlayerStanceKind::Crouched {
            if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
                state.stand_requested = true;
                state.target_eye_height = tuning.crouched_camera_eye_height;
            }
        }

        let jump_requested = actions.jump_pressed;
        let grounded = world
            .get::<PlayerGroundState>(player)
            .map(|state| state.grounded)
            .unwrap_or(false);
        if jump_requested && grounded && tuning.jump_speed > 0.0 {
            let mut velocity = world.get::<Velocity>(player).copied().unwrap_or_default();
            velocity.0.y = tuning.jump_speed;
            let _ = world.insert(player, velocity);
            if let Some(state) = world.get_mut::<PlayerGroundState>(player) {
                state.grounded = false;
                state.walkable = false;
                state.ground_entity = None;
                state.distance = f32::INFINITY;
            }
        }
    }

    update_player_stance_camera(world, dt, tuning);
}

/// Changes the capsule while preserving the world-space foot plane.
///
/// The player origin is the capsule center. Adjusting it by the half-height delta prevents
/// crouching from sinking through the floor and standing from lifting the feet off the ground.
pub fn apply_player_stance_geometry(
    world: &mut World,
    player: EntityId,
    target: PlayerStanceKind,
    tuning: FpsPlayerTuning,
    fixed_tick: u64,
) -> bool {
    let tuning = tuning.sanitized();
    let Some(mut body) = world.get::<PhysicsBodyDesc>(player).copied() else {
        return false;
    };
    let CollisionShapeDesc::Capsule {
        radius,
        half_height: current_half_height,
    } = body.shape
    else {
        return false;
    };
    let target_half_height = match target {
        PlayerStanceKind::Standing => tuning.body_half_height,
        PlayerStanceKind::Crouched => tuning.crouched_body_half_height,
    };
    let delta_y = target_half_height - current_half_height;

    if delta_y.abs() > 1.0e-6 {
        if let Some(transform) = world.get_mut::<Transform>(player) {
            transform.position.y += delta_y;
        }
        body.shape = CollisionShapeDesc::Capsule {
            radius,
            half_height: target_half_height,
        };
        ensure_physics_body(world, player, body);
    }

    if world.get::<PlayerStanceState>(player).is_none() {
        let _ = world.insert(
            player,
            PlayerStanceState::standing(tuning.camera_eye_height),
        );
    }
    if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
        // Compensate the center shift so the camera's absolute Y starts continuous, then let the
        // fixed-step camera filter converge to the new stance eye height.
        state.current_eye_height = (state.current_eye_height - delta_y).clamp(0.0, 20.0);
        state.current = target;
        state.stand_requested = false;
        state.stand_blocked = false;
        state.target_eye_height = match target {
            PlayerStanceKind::Standing => tuning.camera_eye_height,
            PlayerStanceKind::Crouched => tuning.crouched_camera_eye_height,
        };
        state.last_transition_tick = fixed_tick;
    }

    emit_player_event(
        world,
        player,
        PlayerEventKind::StanceChanged,
        match target {
            PlayerStanceKind::Standing => "stance=standing",
            PlayerStanceKind::Crouched => "stance=crouched",
        },
    );
    true
}

pub fn update_player_stance_camera(world: &mut World, dt: f32, tuning: FpsPlayerTuning) {
    let tuning = tuning.sanitized();
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let alpha = if dt > 0.0 {
        1.0 - (-tuning.crouch_camera_speed * dt).exp()
    } else {
        1.0
    };
    let players = world
        .query::<PlayerStanceState>()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for player in players {
        if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
            state.current_eye_height +=
                (state.target_eye_height - state.current_eye_height) * alpha.clamp(0.0, 1.0);
            if (state.target_eye_height - state.current_eye_height).abs() < 1.0e-4 {
                state.current_eye_height = state.target_eye_height;
            }
        }
    }

    let cameras = world
        .query::<FollowTargetCameraController>()
        .map(|(id, ctrl)| (id, ctrl.target))
        .collect::<Vec<_>>();
    for (camera, target) in cameras {
        let Some(eye_height) = world
            .get::<PlayerStanceState>(target)
            .map(|state| state.current_eye_height)
        else {
            continue;
        };
        if let Some(controller) = world.get_mut::<FollowTargetCameraController>(camera) {
            controller.offset_ls.y = eye_height;
        }
    }
}

/// Consumes render-frame pulses after one fixed simulation step.
pub fn consume_player_transient_input(world: &mut World) {
    let players = world
        .query2_ids::<PlayerController, MotorInput>()
        .collect::<Vec<_>>();
    for player in players {
        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.look_delta = Vec2::ZERO;
            input.zoom_delta = 0.0;
        }
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.clear_pulses();
        }
    }
}

#[inline]
pub fn attach_active_camera_to_player(world: &mut World, camera: EntityId, player: EntityId) {
    if !world.exists(camera) || !world.exists(player) {
        return;
    }

    let ctrl = world
        .get::<FollowTargetCameraController>(camera)
        .copied()
        .unwrap_or(FollowTargetCameraController {
            target: player,
            offset_ls: Vec3::new(0.0, 1.6, 4.5),
            rot_offset: Quat::IDENTITY,
            follow_rotation: false,
            smooth_time: 0.08,
            max_speed: 0.0,
        });

    let mut next = ctrl;
    next.target = player;
    let eye_height = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.camera_eye_height)
        .unwrap_or_else(|| FpsPlayerTuning::default().camera_eye_height);
    next.offset_ls = Vec3::new(0.0, eye_height, 0.0);
    next.rot_offset = Quat::IDENTITY;
    next.follow_rotation = true;
    next.smooth_time = 0.0;
    next.max_speed = 0.0;

    let _ = world.insert(camera, next);
    let _ = world.insert(camera, FollowTargetCameraMotor::default());

    if world.get::<CameraRigComp>(camera).is_none() {
        let rig = world
            .get::<Transform>(camera)
            .copied()
            .map(|t| {
                CameraRigComp(newengine_camera::CameraRig {
                    position: t.position,
                    rotation: t.rotation,
                })
            })
            .unwrap_or_default();
        let _ = world.insert(camera, rig);
    }

    emit_player_event(world, player, PlayerEventKind::Possessed, "camera attached");
}

#[inline]
pub fn detach_active_camera_from_player(world: &mut World, camera: EntityId) {
    let target = world
        .get::<FollowTargetCameraController>(camera)
        .map(|ctrl| ctrl.target);
    let _ = world.remove::<FollowTargetCameraController>(camera);
    let _ = world.remove::<FollowTargetCameraMotor>(camera);
    if let Some(player) = target {
        emit_player_event(world, player, PlayerEventKind::Released, "camera detached");
    }
}

#[inline]
pub fn display_visible_in_mode(world: &World, entity: EntityId, runtime: bool) -> bool {
    let vis = world
        .get::<DisplayVisibility>(entity)
        .copied()
        .unwrap_or_default();
    // RuntimeHidden is a hard presentation quarantine. This is important during
    // loading / first-world handoff, where the render controller may still use
    // a non-runtime extraction path while the camera is already first-person.
    // First-person avatar bodies and fallback capsules must not leak as white
    // diagnostic silhouettes in the center of the screen.
    if matches!(vis.mode, DisplayMode::RuntimeHidden) {
        return false;
    }
    if runtime {
        vis.visible_in_game()
    } else {
        vis.visible_in_authoring()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_input_actions_api::GameplayActionFrame;

    #[test]
    fn player_command_handoff_preserves_frame_sequence_and_actions() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "test-player", Vec3::ZERO);
        let actions = GameplayActionFrame {
            jump_pressed: true,
            fire_primary_held: true,
            ..GameplayActionFrame::default()
        };

        apply_player_command_frame(&mut world, player, 42, actions);

        assert_eq!(
            world.get::<PlayerCommandFrame>(player).copied(),
            Some(PlayerCommandFrame::new(42, actions))
        );
    }

    #[test]
    fn transient_input_is_buffered_until_one_fixed_step_consumes_it() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "test-player", Vec3::ZERO);
        apply_player_input(
            &mut world,
            player,
            input_move::FORWARD,
            Vec2::new(2.0, -1.0),
            true,
        );
        apply_player_input(
            &mut world,
            player,
            input_move::FORWARD,
            Vec2::new(3.0, 4.0),
            true,
        );
        apply_player_command_frame(
            &mut world,
            player,
            7,
            GameplayActionFrame {
                jump_pressed: true,
                fire_primary_held: true,
                ..GameplayActionFrame::default()
            },
        );
        apply_player_command_frame(
            &mut world,
            player,
            8,
            GameplayActionFrame {
                reload_pressed: true,
                fire_primary_held: false,
                aim_held: true,
                ..GameplayActionFrame::default()
            },
        );

        assert_eq!(
            world
                .get::<MotorInput>(player)
                .map(|input| input.look_delta),
            Some(Vec2::new(5.0, 3.0))
        );
        let pending = world
            .get::<PlayerCommandFrame>(player)
            .copied()
            .expect("player command frame");
        assert_eq!(pending.source_frame, 8);
        assert!(pending.actions.jump_pressed);
        assert!(pending.actions.reload_pressed);
        assert!(!pending.actions.fire_primary_held);
        assert!(pending.actions.aim_held);

        consume_player_transient_input(&mut world);

        assert_eq!(
            world
                .get::<MotorInput>(player)
                .map(|input| input.look_delta),
            Some(Vec2::ZERO)
        );
        let consumed = world
            .get::<PlayerCommandFrame>(player)
            .copied()
            .expect("player command frame");
        assert!(!consumed.actions.jump_pressed);
        assert!(!consumed.actions.reload_pressed);
        assert!(consumed.actions.aim_held);
    }

    #[test]
    fn grounded_jump_command_sets_vertical_velocity_once() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "jump-player", Vec3::ZERO);
        let jump_speed = FpsPlayerTuning::default().jump_speed;
        if let Some(state) = world.get_mut::<PlayerGroundState>(player) {
            state.grounded = true;
            state.ground_entity = Some(99);
            state.distance = 0.02;
        }
        apply_player_command_frame(
            &mut world,
            player,
            10,
            GameplayActionFrame {
                jump_pressed: true,
                ..GameplayActionFrame::default()
            },
        );

        apply_player_fixed_commands(&mut world, 1.0 / 60.0, 1);

        assert_eq!(
            world.get::<Velocity>(player).map(|velocity| velocity.0.y),
            Some(jump_speed)
        );
        assert!(
            !world
                .get::<PlayerGroundState>(player)
                .expect("ground state")
                .grounded
        );

        consume_player_transient_input(&mut world);
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0.y = 2.0;
        }
        apply_player_fixed_commands(&mut world, 1.0 / 60.0, 1);
        assert_eq!(
            world.get::<Velocity>(player).map(|velocity| velocity.0.y),
            Some(2.0)
        );
    }

    #[test]
    fn airborne_jump_request_does_not_change_vertical_velocity() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "airborne-player", Vec3::ZERO);
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0.y = -3.0;
        }
        apply_player_command_frame(
            &mut world,
            player,
            12,
            GameplayActionFrame {
                jump_pressed: true,
                ..GameplayActionFrame::default()
            },
        );

        apply_player_fixed_commands(&mut world, 1.0 / 60.0, 1);

        assert_eq!(
            world.get::<Velocity>(player).map(|velocity| velocity.0.y),
            Some(-3.0)
        );
    }
}
