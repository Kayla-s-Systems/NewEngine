#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::{Aabb, Bounds, Sphere};
use newengine_ecs::{Component, EntityId, World};
use newengine_math::collections::FxHashSet;
use newengine_math::{Quat, Vec2, Vec3};
use newengine_primitives::{builtins as prim_builtins, Primitive};
use newengine_scene::components::Name;
use newengine_sim::{
    default_schedule, AngularVelocity, CameraRigComp, CharacterMotor, FollowTargetCameraController,
    FollowTargetCameraMotor, MotorInput, SimFrame, SimSchedule, Velocity,
};
use newengine_transform::{set_parent, Transform};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EditorPlayMode {
    #[default]
    Edit,
    Simulate,
    Play,
}

impl EditorPlayMode {
    #[inline]
    pub const fn is_runtime(self) -> bool {
        matches!(self, Self::Simulate | Self::Play)
    }

    #[inline]
    pub const fn runs_physics(self) -> bool {
        self.is_runtime()
    }

    #[inline]
    pub const fn wants_direct_player_control(self) -> bool {
        matches!(self, Self::Play)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CollisionShape {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl Default for CollisionShape {
    #[inline]
    fn default() -> Self {
        Self::Box {
            half_extents: [0.5, 0.5, 0.5],
        }
    }
}

impl CollisionShape {
    #[inline]
    pub fn local_aabb(self) -> Aabb {
        match self {
            CollisionShape::Box { half_extents } => Aabb::from_center_half_extents(
                Vec3::ZERO,
                Vec3::new(half_extents[0], half_extents[1], half_extents[2]),
            ),
            CollisionShape::Sphere { radius } => {
                Aabb::from_center_half_extents(Vec3::ZERO, Vec3::splat(radius.max(0.001)))
            }
            CollisionShape::Capsule {
                radius,
                half_height,
            } => {
                let r = radius.max(0.001);
                let hy = half_height.max(0.0) + r;
                Aabb::from_center_half_extents(Vec3::ZERO, Vec3::new(r, hy, r))
            }
        }
    }

    #[inline]
    pub fn local_sphere(self) -> Sphere {
        match self {
            CollisionShape::Box { half_extents } => {
                let he = Vec3::new(half_extents[0], half_extents[1], half_extents[2]);
                Sphere::new(Vec3::ZERO, he.length().max(0.001))
            }
            CollisionShape::Sphere { radius } => Sphere::new(Vec3::ZERO, radius.max(0.001)),
            CollisionShape::Capsule {
                radius,
                half_height,
            } => Sphere::new(Vec3::ZERO, (half_height.max(0.0) + radius.max(0.001)).max(0.001)),
        }
    }

    #[inline]
    pub fn to_bounds(self) -> Bounds {
        match self {
            CollisionShape::Sphere { .. } => Bounds::from_local_sphere(self.local_sphere()),
            _ => Bounds::from_local_aabb(self.local_aabb()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionBody {
    pub shape: CollisionShape,
    pub dynamic: bool,
    pub is_trigger: bool,
}

impl Default for CollisionBody {
    #[inline]
    fn default() -> Self {
        Self {
            shape: CollisionShape::default(),
            dynamic: false,
            is_trigger: false,
        }
    }
}

impl CollisionBody {
    #[inline]
    pub fn to_bounds(self) -> Bounds {
        self.shape.to_bounds()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerActor;

#[derive(Clone, Copy, Debug, Default)]
pub struct GameplayActor;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DisplayMode {
    #[default]
    Both,
    EditorOnly,
    GameOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DisplayVisibility {
    pub mode: DisplayMode,
}

impl DisplayVisibility {
    #[inline]
    pub const fn visible_in_editor(self) -> bool {
        !matches!(self.mode, DisplayMode::GameOnly)
    }

    #[inline]
    pub const fn visible_in_game(self) -> bool {
        !matches!(self.mode, DisplayMode::EditorOnly)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeEntitySnapshot {
    pub entity: EntityId,
    pub transform: Option<Transform>,
    pub velocity: Option<Velocity>,
    pub angular_velocity: Option<AngularVelocity>,
    pub motor_input: Option<MotorInput>,
    pub character_motor: Option<CharacterMotor>,
    pub camera_rig: Option<CameraRigComp>,
    pub follow_controller: Option<FollowTargetCameraController>,
    pub follow_motor: Option<FollowTargetCameraMotor>,
    pub collision_body: Option<CollisionBody>,
    pub display_visibility: Option<DisplayVisibility>,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeWorldSnapshot {
    pub entities: Vec<RuntimeEntitySnapshot>,
}


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
    let e = world.spawn();

    let _ = world.insert(e, Name(name.into()));
    let _ = world.insert(
        e,
        Transform {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(0.45, 0.9, 0.45),
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
                radius: 0.45,
                half_height: 0.45,
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

    if let Some(input) = world.get_mut::<MotorInput>(player) {
        input.move_axis = axis;
        input.look_delta = look_delta_px;
        input.look_active = look_active;
        input.speed_mul = if move_mask & newengine_viewport::input::MOVE_SHIFT != 0 {
            1.75
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
    next.offset_ls = Vec3::new(0.0, 0.85, 0.0);
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

#[inline]
fn translate_aabb(aabb: Aabb, delta: Vec3) -> Aabb {
    Aabb::new(aabb.min + delta, aabb.max + delta)
}

#[inline]
fn minimal_separation(a: &Aabb, b: &Aabb) -> Option<Vec3> {
    if !a.intersects(b) {
        return None;
    }

    let overlap_x = (a.max.x - b.min.x).min(b.max.x - a.min.x);
    let overlap_y = (a.max.y - b.min.y).min(b.max.y - a.min.y);
    let overlap_z = (a.max.z - b.min.z).min(b.max.z - a.min.z);

    let ac = a.center();
    let bc = b.center();

    if overlap_x <= overlap_y && overlap_x <= overlap_z {
        let sx = if ac.x < bc.x { -overlap_x } else { overlap_x };
        Some(Vec3::new(sx, 0.0, 0.0))
    } else if overlap_y <= overlap_z {
        let sy = if ac.y < bc.y { -overlap_y } else { overlap_y };
        Some(Vec3::new(0.0, sy, 0.0))
    } else {
        let sz = if ac.z < bc.z { -overlap_z } else { overlap_z };
        Some(Vec3::new(0.0, 0.0, sz))
    }
}

#[inline]
pub fn capture_runtime_world_snapshot(world: &World) -> RuntimeWorldSnapshot {
    let mut entities: Vec<RuntimeEntitySnapshot> = world
        .iter_entities()
        .map(|entity| RuntimeEntitySnapshot {
            entity,
            transform: world.get::<Transform>(entity).copied(),
            velocity: world.get::<Velocity>(entity).copied(),
            angular_velocity: world.get::<AngularVelocity>(entity).copied(),
            motor_input: world.get::<MotorInput>(entity).copied(),
            character_motor: world.get::<CharacterMotor>(entity).copied(),
            camera_rig: world.get::<CameraRigComp>(entity).copied(),
            follow_controller: world.get::<FollowTargetCameraController>(entity).copied(),
            follow_motor: world.get::<FollowTargetCameraMotor>(entity).copied(),
            collision_body: world.get::<CollisionBody>(entity).copied(),
            display_visibility: world.get::<DisplayVisibility>(entity).copied(),
        })
        .collect();
    entities.sort_by_key(|it| it.entity.stable_u64());
    RuntimeWorldSnapshot { entities }
}

#[inline]
fn restore_component_opt<T: Component + Copy>(world: &mut World, entity: EntityId, value: Option<T>) {
    if let Some(v) = value {
        let _ = world.insert(entity, v);
    } else {
        let _ = world.remove::<T>(entity);
    }
}

#[inline]
pub fn restore_runtime_world_snapshot(world: &mut World, snapshot: RuntimeWorldSnapshot) {
    let live_ids: Vec<EntityId> = world.iter_entities().collect();
    let original_ids: FxHashSet<EntityId> = snapshot.entities.iter().map(|it| it.entity).collect();

    for entity in live_ids {
        if !original_ids.contains(&entity) {
            let _ = world.despawn(entity);
        }
    }

    for entry in snapshot.entities {
        if !world.exists(entry.entity) {
            continue;
        }

        restore_component_opt(world, entry.entity, entry.transform);
        restore_component_opt(world, entry.entity, entry.velocity);
        restore_component_opt(world, entry.entity, entry.angular_velocity);
        restore_component_opt(world, entry.entity, entry.motor_input);
        restore_component_opt(world, entry.entity, entry.character_motor);
        restore_component_opt(world, entry.entity, entry.camera_rig);
        restore_component_opt(world, entry.entity, entry.follow_controller);
        restore_component_opt(world, entry.entity, entry.follow_motor);
        restore_component_opt(world, entry.entity, entry.collision_body);
        restore_component_opt(world, entry.entity, entry.display_visibility);
    }
}

#[inline]
fn step_runtime_physics(world: &mut World, dt: f32) {
    let dt = dt.clamp(0.0001, 0.05);
    let gravity = 9.81_f32;

    let mut static_colliders: Vec<(EntityId, Aabb)> = world
        .query2::<CollisionBody, Bounds>()
        .filter_map(|(entity, body, bounds)| {
            if body.dynamic || body.is_trigger {
                None
            } else {
                Some((entity, bounds.world_aabb))
            }
        })
        .collect();
    static_colliders.sort_by_key(|it| it.0.stable_u64());

    let mut dynamic_ids: Vec<EntityId> = world
        .query::<CollisionBody>()
        .filter_map(|(entity, body)| (body.dynamic && !body.is_trigger).then_some(entity))
        .collect();
    dynamic_ids.sort_by_key(|id| id.stable_u64());

    for entity in dynamic_ids {
        let Some(body) = world.get::<CollisionBody>(entity).copied() else {
            continue;
        };
        let Some(transform) = world.get::<Transform>(entity).copied() else {
            continue;
        };

        let mut velocity = world.get::<Velocity>(entity).copied().unwrap_or_default();
        velocity.0.y -= gravity * dt;

        let mut next_pos = transform.position;
        next_pos.y += velocity.0.y * dt;

        let local_aabb = body.shape.local_aabb();
        let mut world_aabb = translate_aabb(local_aabb, next_pos);

        for (other, static_aabb) in &static_colliders {
            if *other == entity {
                continue;
            }
            let Some(push) = minimal_separation(&world_aabb, static_aabb) else {
                continue;
            };

            next_pos = next_pos + push;
            world_aabb = translate_aabb(world_aabb, push);

            if push.y != 0.0 {
                velocity.0.y = 0.0;
            }
            if push.x != 0.0 {
                velocity.0.x = 0.0;
            }
            if push.z != 0.0 {
                velocity.0.z = 0.0;
            }
        }

        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = next_pos;
        }
        let _ = world.insert(entity, velocity);
    }
}

#[inline]
pub fn run_schedule(schedule: &mut SimSchedule, world: &mut World, dt: f32) {
    let frame = SimFrame::new(dt.max(0.0001), 0);
    schedule.run_default_pipeline(world, frame);
    step_runtime_physics(world, frame.dt);
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}
