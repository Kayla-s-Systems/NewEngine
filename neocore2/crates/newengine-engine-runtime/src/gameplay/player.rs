use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::move_mask as input_move;
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
    CollisionShapeDesc, DisplayMode, DisplayVisibility, FpsDemoRules, FpsPlayerTuning,
    GameplayActor, PhysicsBodyDesc, PlayerActor, PlayerController, PlayerEventKind,
    PlayerModelBinding, PlayerViewVisibility, PlayerVisualKind, PlayerVisualPart,
};

#[inline]
pub fn ensure_physics_body(world: &mut World, entity: EntityId, body: PhysicsBodyDesc) {
    let _ = world.insert(entity, body);
    let _ = world.insert(entity, body.to_bounds());
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
            scale: Vec3::new(tuning.visual_radius, tuning.visual_half_height, tuning.visual_radius),
        },
    );
    let _ = world.insert(visual, Primitive {
        id: prim_builtins::ID_CAPSULE,
        color: [0.30, 0.72, 0.98, 1.0],
    });
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
        input.look_delta = look_delta_px;
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
        emit_player_event(world, player, PlayerEventKind::InputApplied, "local input sampled");
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
            .map(|t| CameraRigComp(newengine_camera::CameraRig {
                position: t.position,
                rotation: t.rotation,
            }))
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
