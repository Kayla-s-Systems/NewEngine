#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_lighting::AmbientLight;
use newengine_materials::{MaterialDescriptor, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_primitives::{builtins, Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::{spawn_named, Scene};
use newengine_transform::Transform;

use crate::gameplay::{
    ensure_collision_body, spawn_default_player, CollisionBody, CollisionShape, DisplayMode,
    DisplayVisibility, FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoState,
};
use crate::scene_bootstrap::bootstrap_editor_scene;

use super::helpers::{
    apply_primitive_instance, ensure_primitive_base, ensure_root, primitive_bounds,
};

#[inline]
pub(super) fn game_ready_demo_enabled() -> bool {
    std::env::var("NEWENGINE_GAME_READY_DEMO")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            !matches!(v.as_str(), "" | "0" | "false" | "off" | "no")
        })
        .unwrap_or(false)
}

#[inline]
pub(super) fn spawn_game_primitive(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    default_mat: MaterialId,
    root: EntityId,
    id: PrimitiveId,
    name: &'static str,
    position: Vec3,
    scale: Vec3,
    color: [f32; 4],
) -> EntityId {
    let entity = spawn_named(world, name);
    let _ = newengine_transform::set_parent(world, entity, Some(root));
    let _ = world.insert(entity, Primitive { id, color });

    if let Some(bounds) = primitive_bounds(prims, id) {
        let _ = world.insert(entity, bounds);
    }

    ensure_primitive_base(world, entity, default_mat);
    apply_primitive_instance(world, mats, entity, default_mat, color);

    if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
        t.position = position;
        t.scale = scale;
    }

    entity
}

pub(super) fn bootstrap_fps_game_ready_scene(
    scene: &mut Scene,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
) -> Option<EntityId> {
    *scene = Scene::new();
    bootstrap_editor_scene(scene);

    let root = ensure_root(scene);
    let active_camera = scene.active_camera();
    let default_mat = mats.register_named("Default", MaterialDescriptor::default());
    let world = scene.world_mut();

    world.insert_resource(FpsDemoState::new(3));

    match world.resource_mut::<AmbientLight>() {
        Some(a) => {
            a.color = [0.35, 0.42, 0.55];
            a.intensity = 0.62;
        }
        None => world.insert_resource(AmbientLight {
            color: [0.35, 0.42, 0.55],
            intensity: 0.62,
        }),
    }

    let floor = spawn_game_primitive(
        world,
        prims,
        mats,
        default_mat,
        root,
        builtins::ID_CUBE,
        "Arena/Floor",
        Vec3::new(0.0, -0.15, 0.0),
        Vec3::new(18.0, 0.15, 18.0),
        [0.12, 0.14, 0.16, 1.0],
    );
    ensure_collision_body(
        world,
        floor,
        CollisionBody {
            shape: CollisionShape::Box {
                half_extents: [18.0, 0.15, 18.0],
            },
            dynamic: false,
            is_trigger: false,
        },
    );

    let walls = [
        ("Arena/Wall-N", Vec3::new(0.0, 1.0, -18.0), Vec3::new(18.0, 1.0, 0.25)),
        ("Arena/Wall-S", Vec3::new(0.0, 1.0, 18.0), Vec3::new(18.0, 1.0, 0.25)),
        ("Arena/Wall-W", Vec3::new(-18.0, 1.0, 0.0), Vec3::new(0.25, 1.0, 18.0)),
        ("Arena/Wall-E", Vec3::new(18.0, 1.0, 0.0), Vec3::new(0.25, 1.0, 18.0)),
    ];
    for (name, pos, scale) in walls {
        let wall = spawn_game_primitive(
            world,
            prims,
            mats,
            default_mat,
            root,
            builtins::ID_CUBE,
            name,
            pos,
            scale,
            [0.20, 0.22, 0.28, 1.0],
        );
        ensure_collision_body(
            world,
            wall,
            CollisionBody {
                shape: CollisionShape::Box {
                    half_extents: [scale.x, scale.y, scale.z],
                },
                dynamic: false,
                is_trigger: false,
            },
        );
    }

    let blockers = [
        ("Arena/Crate-A", Vec3::new(-5.0, 0.55, -4.0), Vec3::new(1.25, 0.55, 2.25)),
        ("Arena/Crate-B", Vec3::new(3.5, 0.55, -1.5), Vec3::new(2.0, 0.55, 1.0)),
        ("Arena/Crate-C", Vec3::new(-1.0, 0.55, 5.0), Vec3::new(1.0, 0.55, 2.6)),
        ("Arena/Crate-D", Vec3::new(7.5, 0.55, 6.0), Vec3::new(1.6, 0.55, 1.6)),
    ];
    for (name, pos, scale) in blockers {
        let blocker = spawn_game_primitive(
            world,
            prims,
            mats,
            default_mat,
            root,
            builtins::ID_CUBE,
            name,
            pos,
            scale,
            [0.30, 0.31, 0.36, 1.0],
        );
        ensure_collision_body(
            world,
            blocker,
            CollisionBody {
                shape: CollisionShape::Box {
                    half_extents: [scale.x, scale.y, scale.z],
                },
                dynamic: false,
                is_trigger: false,
            },
        );
    }

    let pickups = [
        ("Core/Blue-01", Vec3::new(-9.5, 0.75, -8.0)),
        ("Core/Blue-02", Vec3::new(6.0, 0.75, -7.5)),
        ("Core/Blue-03", Vec3::new(-6.5, 0.75, 8.0)),
    ];
    for (name, pos) in pickups {
        let pickup = spawn_game_primitive(
            world,
            prims,
            mats,
            default_mat,
            root,
            builtins::ID_SPHERE_UV,
            name,
            pos,
            Vec3::splat(0.42),
            [0.15, 0.68, 1.0, 1.0],
        );
        let _ = world.insert(pickup, FpsDemoPickup { radius: 1.15 });
        let _ = world.insert(
            pickup,
            CollisionBody {
                shape: CollisionShape::Sphere { radius: 0.6 },
                dynamic: false,
                is_trigger: true,
            },
        );
    }

    let hazards = [
        ("Hazard/Purple-01", Vec3::new(0.0, 0.2, -8.5)),
        ("Hazard/Purple-02", Vec3::new(8.5, 0.2, 1.5)),
        ("Hazard/Purple-03", Vec3::new(-9.0, 0.2, 2.5)),
    ];
    for (name, pos) in hazards {
        let hazard = spawn_game_primitive(
            world,
            prims,
            mats,
            default_mat,
            root,
            builtins::ID_CYLINDER,
            name,
            pos,
            Vec3::new(1.15, 0.08, 1.15),
            [0.75, 0.22, 1.0, 1.0],
        );
        let _ = world.insert(hazard, FpsDemoHazard { radius: 1.25 });
        let _ = world.insert(
            hazard,
            CollisionBody {
                shape: CollisionShape::Sphere { radius: 1.25 },
                dynamic: false,
                is_trigger: true,
            },
        );
    }

    let goal = spawn_game_primitive(
        world,
        prims,
        mats,
        default_mat,
        root,
        builtins::ID_CYLINDER,
        "Extraction/Red-Beacon",
        Vec3::new(11.5, 0.55, 11.5),
        Vec3::new(0.85, 0.55, 0.85),
        [1.0, 0.20, 0.14, 1.0],
    );
    let _ = world.insert(goal, FpsDemoGoal { radius: 1.6 });
    let _ = world.insert(
        goal,
        CollisionBody {
            shape: CollisionShape::Sphere { radius: 1.6 },
            dynamic: false,
            is_trigger: true,
        },
    );

    let player = spawn_default_player(
        world,
        Some(root),
        "Player/FPS",
        Vec3::new(-12.5, 0.95, -12.5),
    );
    let _ = world.insert(
        player,
        DisplayVisibility {
            mode: DisplayMode::EditorOnly,
        },
    );
    if let Some(motor) = world.get_mut::<newengine_sim::CharacterMotor>(player) {
        motor.move_speed = 7.0;
        motor.look_sens = 0.0022;
        motor.yaw = -0.78;
    }
    if let Some(t) = world.get_mut_tracked::<Transform>(player) {
        t.rotation = Quat::from_euler(EulerRot::YXZ, -0.78, 0.0, 0.0);
    }

    if let Some(cam) = active_camera {
        if let Some(t) = world.get_mut_tracked::<Transform>(cam) {
            t.position = Vec3::new(-12.5, 1.8, -12.5);
            t.rotation = Quat::from_euler(EulerRot::YXZ, -0.78, 0.0, 0.0);
        }
    }

    let _ = scene.validate_invariants();
    Some(player)
}
