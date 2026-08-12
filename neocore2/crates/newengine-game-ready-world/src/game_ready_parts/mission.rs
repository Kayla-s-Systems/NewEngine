use super::foliage::terrain_height;
use super::*;

const MISSION_MATERIAL_LIBRARY: &str = "materials/world_game_ready.nemat";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct GameReadyMissionSpawnSummary {
    pub pickups: u32,
    pub targets: u32,
    pub hazards: u32,
    pub goals: u32,
}

#[derive(Clone, Copy, Debug)]
struct MissionMaterials {
    core: MaterialId,
    target: MaterialId,
    hazard: MaterialId,
    goal: MaterialId,
}

fn mission_material_spec(entry: &str) -> GameReadyMaterialSpec {
    GameReadyMaterialSpec {
        asset: Some(format!("{MISSION_MATERIAL_LIBRARY}@{entry}")),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.3,
        normal_scale: 0.0,
        occlusion_strength: 1.0,
    }
}

fn register_mission_materials(mats: &MaterialRegistry) -> MissionMaterials {
    let core_spec = mission_material_spec("mission_core");
    let target_spec = mission_material_spec("mission_target");
    let hazard_spec = mission_material_spec("mission_hazard");
    let goal_spec = mission_material_spec("mission_goal");

    MissionMaterials {
        core: register_material(
            mats,
            "FPS/Mission/Core",
            [0.04, 0.62, 1.0, 1.0],
            [0.02, 0.55, 1.0],
            3.2,
            MaterialFlags::DOUBLE_SIDED,
            &core_spec,
        ),
        target: register_material(
            mats,
            "FPS/Mission/Target",
            [1.0, 0.18, 0.04, 1.0],
            [0.72, 0.06, 0.01],
            1.5,
            MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            &target_spec,
        ),
        hazard: register_material(
            mats,
            "FPS/Mission/Hazard",
            [0.96, 0.02, 0.08, 1.0],
            [1.0, 0.01, 0.04],
            3.8,
            MaterialFlags::DOUBLE_SIDED,
            &hazard_spec,
        ),
        goal: register_material(
            mats,
            "FPS/Mission/Goal",
            [0.08, 1.0, 0.34, 1.0],
            [0.04, 1.0, 0.22],
            3.4,
            MaterialFlags::DOUBLE_SIDED,
            &goal_spec,
        ),
    }
}

#[inline]
fn mission_position(
    world: &newengine_ecs::World,
    terrain: EntityId,
    authored: Vec3,
    center_offset: f32,
) -> Vec3 {
    Vec3::new(
        authored.x,
        terrain_height(world, terrain, authored.x, authored.z) + authored.y + center_offset,
        authored.z,
    )
}

fn spawn_mission_primitive(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    material_id: MaterialId,
    primitive_id: PrimitiveId,
    name: &str,
    position: Vec3,
    scale: Vec3,
) -> EntityId {
    spawn_game_primitive(
        world,
        prims,
        mats,
        PrimitiveSpawnSpec {
            parent,
            primitive_id,
            material_id,
            name,
            position,
            scale,
            color: [1.0, 1.0, 1.0, 1.0],
            render_options: newengine_model_domain_api::MeshRenderOptions::world_opaque(),
        },
    )
}

pub(super) fn spawn_game_ready_mission(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    terrain: EntityId,
    mission: &GameReadyMissionSpec,
) -> GameReadyMissionSpawnSummary {
    let mut summary = GameReadyMissionSpawnSummary::default();
    let materials = register_mission_materials(mats);

    for pickup in &mission.pickups {
        let position = mission_position(world, terrain, pickup.position, pickup.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            prims,
            mats,
            parent,
            materials.core,
            builtins::ID_SPHERE_UV,
            &format!("Mission/Pickup/{}", pickup.id),
            position,
            pickup.scale,
        );
        let _ = world.insert(
            entity,
            FpsDemoPickup {
                radius: pickup.radius,
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Interactable::new(format!("Collect {}", pickup.id)),
        );
        summary.pickups = summary.pickups.saturating_add(1);
    }

    for target in &mission.targets {
        let position = mission_position(world, terrain, target.position, target.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            prims,
            mats,
            parent,
            materials.target,
            builtins::ID_CAPSULE,
            &format!("Mission/Target/{}", target.id),
            position,
            target.scale,
        );
        let shape = newengine_engine_runtime::gameplay::CollisionShapeDesc::Capsule {
            radius: target.scale.x.abs().max(target.scale.z.abs()).max(0.1),
            half_height: (target.scale.y.abs() - target.scale.x.abs()).max(0.1),
        };
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PhysicsBodyDesc::static_solid(shape),
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Health::new(target.health),
        );
        let _ = world.insert(entity, FpsDemoTarget);
        summary.targets = summary.targets.saturating_add(1);
    }

    for hazard in &mission.hazards {
        let position = mission_position(world, terrain, hazard.position, hazard.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            prims,
            mats,
            parent,
            materials.hazard,
            builtins::ID_CYLINDER,
            &format!("Mission/Hazard/{}", hazard.id),
            position,
            hazard.scale,
        );
        let _ = world.insert(
            entity,
            FpsDemoHazard {
                radius: hazard.radius,
            },
        );
        summary.hazards = summary.hazards.saturating_add(1);
    }

    for goal in &mission.goals {
        let position = mission_position(world, terrain, goal.position, goal.scale.y.abs() * 0.15);
        let entity = spawn_mission_primitive(
            world,
            prims,
            mats,
            parent,
            materials.goal,
            builtins::ID_TORUS,
            &format!("Mission/Goal/{}", goal.id),
            position,
            goal.scale,
        );
        let _ = world.insert(
            entity,
            FpsDemoGoal {
                radius: goal.radius,
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Interactable::new(format!(
                "Extract at {}",
                goal.id
            )),
        );
        summary.goals = summary.goals.saturating_add(1);
    }

    newengine_ulog_api::ulog::info!(
        "game-ready mission spawned: pickups={} targets={} hazards={} goals={} materials='{}@mission_*' policy='authored .ymap mission -> ordinary ECS render/physics/gameplay entities'",
        summary.pickups,
        summary.targets,
        summary.hazards,
        summary.goals,
        MISSION_MATERIAL_LIBRARY,
    );
    summary
}
