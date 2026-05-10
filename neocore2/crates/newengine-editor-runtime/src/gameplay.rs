#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::{Aabb, Bounds, Sphere};
use newengine_ecs::{Component, EntityId, World};
use newengine_math::collections::FxHashSet;
use newengine_math::{Mat4, Quat, Vec2, Vec3};
use newengine_primitives::{builtins as prim_builtins, Primitive};
use newengine_procedural_noise::ProceduralTerrain;
use newengine_scene::components::Name;
use newengine_sim::{
    default_schedule, AngularVelocity, CameraRigComp, CharacterMotor, FollowTargetCameraController,
    FollowTargetCameraMotor, MotorInput, SimFrame, SimSchedule, Velocity,
};
use newengine_transform::{set_parent, GlobalTransform, Transform};

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

/// Declarative FPS runtime tuning.
///
/// The scene/profile owns these values; runtime systems only consume the resource.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsPlayerTuning {
    pub body_radius: f32,
    pub body_half_height: f32,
    pub visual_radius: f32,
    pub visual_half_height: f32,
    pub camera_eye_height: f32,
    pub sprint_multiplier: f32,
    pub gravity: f32,
    pub contact_skin: f32,
}

impl Default for FpsPlayerTuning {
    #[inline]
    fn default() -> Self {
        Self {
            body_radius: 0.45,
            body_half_height: 0.45,
            visual_radius: 0.45,
            visual_half_height: 0.90,
            camera_eye_height: 0.85,
            sprint_multiplier: 1.75,
            gravity: 9.81,
            contact_skin: 0.035,
        }
    }
}

impl FpsPlayerTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            body_radius: self.body_radius.clamp(0.05, 5.0),
            body_half_height: self.body_half_height.clamp(0.05, 8.0),
            visual_radius: self.visual_radius.clamp(0.05, 8.0),
            visual_half_height: self.visual_half_height.clamp(0.05, 12.0),
            camera_eye_height: self.camera_eye_height.clamp(0.05, 12.0),
            sprint_multiplier: self.sprint_multiplier.clamp(1.0, 8.0),
            gravity: self.gravity.clamp(0.0, 80.0),
            contact_skin: self.contact_skin.clamp(0.0, 0.50),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FpsDemoRules {
    pub default_status: String,
    pub pickup_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
    pub player: FpsPlayerTuning,
}

impl Default for FpsDemoRules {
    #[inline]
    fn default() -> Self {
        Self {
            default_status: "Find blue cores, avoid hazards, reach extraction.".to_string(),
            pickup_status: "Core acquired.".to_string(),
            hazard_status: "You touched a hazard. Relaunch the demo to retry.".to_string(),
            goal_locked_status: "Beacon locked: collect all cores first.".to_string(),
            goal_complete_status: "Extraction complete. Stable runtime loop is playable.".to_string(),
            failed_progress_label: "FAILED — touch a hazard to retry scene".to_string(),
            completed_progress_label: "EXTRACTED".to_string(),
            player: FpsPlayerTuning::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoPickup {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoGoal {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoHazard {
    pub radius: f32,
}

#[cfg_attr(not(feature = "editor-ui"), allow(dead_code))]
#[derive(Clone, Debug)]
pub struct FpsDemoState {
    pub title: String,
    pub objective: String,
    pub elapsed_sec: f32,
    pub pickups_collected: u32,
    pub pickups_total: u32,
    pub completed: bool,
    pub failed: bool,
    pub status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
}

#[cfg_attr(not(feature = "editor-ui"), allow(dead_code))]
impl FpsDemoState {
    #[inline]
    pub fn new(pickups_total: u32) -> Self {
        Self::from_rules(
            pickups_total,
            "KAYLA FPS: Extraction Yard",
            "Collect cores and reach the extraction beacon",
            &FpsDemoRules::default(),
        )
    }

    #[inline]
    pub fn from_rules(
        pickups_total: u32,
        title: impl Into<String>,
        objective: impl Into<String>,
        rules: &FpsDemoRules,
    ) -> Self {
        Self {
            title: title.into(),
            objective: objective.into(),
            elapsed_sec: 0.0,
            pickups_collected: 0,
            pickups_total,
            completed: false,
            failed: false,
            status: rules.default_status.clone(),
            failed_progress_label: rules.failed_progress_label.clone(),
            completed_progress_label: rules.completed_progress_label.clone(),
        }
    }

    #[inline]
    pub fn progress_label(&self) -> String {
        if self.completed {
            return format!("{} in {:.1}s", self.completed_progress_label, self.elapsed_sec.max(0.0));
        }
        if self.failed {
            return self.failed_progress_label.clone();
        }
        format!(
            "Cores {}/{} · {:.1}s",
            self.pickups_collected.min(self.pickups_total),
            self.pickups_total,
            self.elapsed_sec.max(0.0)
        )
    }
}

impl Default for FpsDemoState {
    #[inline]
    fn default() -> Self {
        Self::new(0)
    }
}

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

#[derive(Clone)]
struct RuntimeTerrainSurface {
    key: u64,
    terrain: ProceduralTerrain,
    world_from_local: Mat4,
    local_from_world: Mat4,
}

#[inline]
fn collect_runtime_terrain_surfaces(world: &World) -> Vec<RuntimeTerrainSurface> {
    let mut terrains: Vec<RuntimeTerrainSurface> = world
        .query2::<ProceduralTerrain, GlobalTransform>()
        .map(|(entity, terrain, gt)| RuntimeTerrainSurface {
            key: entity.stable_u64(),
            terrain: terrain.clone(),
            world_from_local: gt.0,
            local_from_world: gt.0.inverse(),
        })
        .collect();
    terrains.sort_by_key(|it| it.key);
    terrains
}

#[inline]
fn resolve_heightfield_contact(
    terrains: &[RuntimeTerrainSurface],
    body: CollisionBody,
    next_pos: &mut Vec3,
    velocity: &mut Velocity,
    contact_skin: f32,
) {
    let local_aabb = body.shape.local_aabb();
    let contact_skin = contact_skin.clamp(0.0, 0.50);

    for surface in terrains {
        let local_pos = surface.local_from_world.transform_point3(*next_pos);
        let Some(local_ground_y) = surface
            .terrain
            .heightfield
            .sample_height_local_checked(local_pos.x, local_pos.z, 0.08)
        else {
            continue;
        };

        let world_ground = surface
            .world_from_local
            .transform_point3(Vec3::new(local_pos.x, local_ground_y, local_pos.z));
        let bottom_y = next_pos.y + local_aabb.min.y;
        let penetration = world_ground.y + contact_skin - bottom_y;
        if penetration <= 0.0 || !penetration.is_finite() {
            continue;
        }

        next_pos.y += penetration;
        if velocity.0.y < 0.0 {
            velocity.0.y = 0.0;
        }
    }
}

#[inline]
fn step_runtime_physics(world: &mut World, dt: f32) {
    let dt = dt.clamp(0.0001, 0.05);
    let player_tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_default();
    let gravity = player_tuning.gravity;
    let contact_skin = player_tuning.contact_skin;

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

    // Exact heightfield contact is engine-side now. The coarse terrain AABB tiles remain
    // useful as editor debug proxies, but runtime locomotion resolves against the actual
    // generated surface so the player does not float on tile maxima or fall through seams.
    let terrain_surfaces = collect_runtime_terrain_surfaces(world);

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

        resolve_heightfield_contact(&terrain_surfaces, body, &mut next_pos, &mut velocity, contact_skin);

        if let Some(t) = world.get_mut::<Transform>(entity) {
            t.position = next_pos;
        }
        let _ = world.insert(entity, velocity);
    }
}


#[inline]
fn distance_sq(a: Vec3, b: Vec3) -> f32 {
    let d = a - b;
    d.length_squared()
}

pub fn step_fps_demo_gameplay(world: &mut World, dt: f32) {
    if world.resource::<FpsDemoState>().is_none() {
        return;
    }

    let terminal = world
        .resource::<FpsDemoState>()
        .map(|s| s.completed || s.failed)
        .unwrap_or(false);

    if !terminal {
        if let Some(state) = world.resource_mut::<FpsDemoState>() {
            if dt.is_finite() && dt > 0.0 {
                state.elapsed_sec += dt.min(0.1);
            }
        }
    }

    let Some(player) = first_player(world) else {
        return;
    };
    let Some(player_pos) = world.get::<Transform>(player).map(|t| t.position) else {
        return;
    };

    if terminal {
        return;
    }

    let mut picked: Vec<EntityId> = Vec::new();
    for (entity, pickup) in world.query::<FpsDemoPickup>() {
        let Some(t) = world.get::<Transform>(entity) else {
            continue;
        };
        let r = pickup.radius.max(0.1);
        if distance_sq(player_pos, t.position) <= r * r {
            picked.push(entity);
        }
    }
    picked.sort_by_key(|id| id.stable_u64());

    for entity in &picked {
        let _ = world.remove::<FpsDemoPickup>(*entity);
        let _ = world.insert(
            *entity,
            DisplayVisibility {
                mode: DisplayMode::EditorOnly,
            },
        );
    }

    let mut hit_hazard = false;
    for (_entity, hazard) in world.query::<FpsDemoHazard>() {
        let Some(t) = world.get::<Transform>(_entity) else {
            continue;
        };
        let r = hazard.radius.max(0.1);
        if distance_sq(player_pos, t.position) <= r * r {
            hit_hazard = true;
            break;
        }
    }

    let mut reached_goal = false;
    for (_entity, goal) in world.query::<FpsDemoGoal>() {
        let Some(t) = world.get::<Transform>(_entity) else {
            continue;
        };
        let r = goal.radius.max(0.1);
        if distance_sq(player_pos, t.position) <= r * r {
            reached_goal = true;
            break;
        }
    }

    let rules = world.resource::<FpsDemoRules>().cloned().unwrap_or_default();
    let collected_delta = picked.len() as u32;
    if let Some(state) = world.resource_mut::<FpsDemoState>() {
        state.pickups_collected = state
            .pickups_collected
            .saturating_add(collected_delta)
            .min(state.pickups_total);

        if hit_hazard {
            state.failed = true;
            state.status = rules.hazard_status.clone();
        } else if reached_goal && state.pickups_collected >= state.pickups_total {
            state.completed = true;
            state.status = rules.goal_complete_status.clone();
        } else if reached_goal {
            state.status = rules.goal_locked_status.clone();
        } else if collected_delta > 0 {
            state.status = rules.pickup_status.clone();
        } else {
            state.status = rules.default_status.clone();
        }
    }
}

#[inline]
pub fn run_schedule(schedule: &mut SimSchedule, world: &mut World, dt: f32) {
    let frame = SimFrame::new(dt.max(0.0001), 0);
    schedule.run_default_pipeline(world, frame);
    step_runtime_physics(world, frame.dt);
    step_fps_demo_gameplay(world, frame.dt);
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}
