use newengine_ecs::{EntityId, World};
use newengine_math::{Quat, Vec2, Vec3};
use newengine_primitives::{builtins as prim_builtins, Primitive};
use newengine_scene::components::Name;
use newengine_sim::{
    CameraRigComp, CharacterMotor, FollowTargetCameraController, FollowTargetCameraMotor,
    MotorInput, Velocity,
};
use newengine_transform::{set_parent, Transform};

use super::{
    CollisionBody, CollisionShape, DisplayVisibility, FpsDemoRules, FpsPlayerTuning,
    GameplayActor, PlayerActor,
};

#[inline]
pub fn ensure_collision_body(world: &mut World, entity: EntityId, body: CollisionBody) {
    let _ = world.insert(entity, body);
    let _ = world.insert(entity, body.to_bounds());
}

#[inline]
pub fn remove_collision_body(world: &mut World, entity: EntityId) {
    let _ = world.remove::<CollisionBody>(entity);
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
    let tuning = tuning.sanitized();
    let e = world.spawn();

    let _ = world.insert(e, Name(name.into()));
    let _ = world.insert(
        e,
        Transform {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(tuning.visual_radius, tuning.visual_half_height, tuning.visual_radius),
        },
    );
    let _ = world.insert(e, Primitive {
        id: prim_builtins::ID_CAPSULE,
        color: [0.30, 0.72, 0.98, 1.0],
    });
    let _ = world.insert(e, GameplayActor);
    let _ = world.insert(e, PlayerActor);
    let _ = world.insert(e, CharacterMotor::default());
    let _ = world.insert(e, MotorInput::default());
    let _ = world.insert(e, Velocity(Vec3::ZERO));

    ensure_collision_body(
        world,
        e,
        CollisionBody {
            shape: CollisionShape::Capsule {
                radius: tuning.body_radius,
                half_height: tuning.body_half_height,
            },
            dynamic: true,
            is_trigger: false,
        },
    );

    if let Some(root) = root.filter(|id| world.exists(*id)) {
        let _ = set_parent(world, e, Some(root));
    }

    e
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
pub fn clear_player_input(world: &mut World, player: EntityId) {
    if let Some(input) = world.get_mut::<MotorInput>(player) {
        *input = MotorInput::default();
    }
}

#[inline]
pub fn apply_player_input(
    world: &mut World,
    player: EntityId,
    move_mask: u64,
    look_delta_px: Vec2,
    look_active: bool,
) {
    let mut axis = Vec3::ZERO;

    if move_mask & newengine_viewport::input::MOVE_W != 0 {
        axis.z += 1.0;
    }
    if move_mask & newengine_viewport::input::MOVE_S != 0 {
        axis.z -= 1.0;
    }
    if move_mask & newengine_viewport::input::MOVE_D != 0 {
        axis.x += 1.0;
    }
    if move_mask & newengine_viewport::input::MOVE_A != 0 {
        axis.x -= 1.0;
    }
    if move_mask & newengine_viewport::input::MOVE_UP != 0 {
        axis.y += 1.0;
    }
    if move_mask & newengine_viewport::input::MOVE_DOWN != 0 {
        axis.y -= 1.0;
    }

    let sprint_multiplier = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sprint_multiplier)
        .unwrap_or_else(|| FpsPlayerTuning::default().sprint_multiplier);

    if let Some(input) = world.get_mut::<MotorInput>(player) {
        input.move_axis = axis;
        input.look_delta = look_delta_px;
        input.look_active = look_active;
        input.speed_mul = if move_mask & newengine_viewport::input::MOVE_SHIFT != 0 {
            sprint_multiplier
        } else {
            1.0
        };
        input.zoom_delta = 0.0;
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
}

#[inline]
pub fn detach_active_camera_from_player(world: &mut World, camera: EntityId) {
    let _ = world.remove::<FollowTargetCameraController>(camera);
    let _ = world.remove::<FollowTargetCameraMotor>(camera);
}

#[inline]
pub fn display_visible_in_mode(world: &World, entity: EntityId, runtime: bool) -> bool {
    let vis = world
        .get::<DisplayVisibility>(entity)
        .copied()
        .unwrap_or_default();
    if runtime {
        vis.visible_in_game()
    } else {
        vis.visible_in_editor()
    }
}
