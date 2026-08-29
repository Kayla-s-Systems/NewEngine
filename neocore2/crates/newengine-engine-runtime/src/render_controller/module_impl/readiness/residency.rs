use newengine_procedural_noise::ProceduralTerrain;

use crate::gameplay::{ModelRenderComponent, PreparedRenderMesh, WorldAssemblyProgress};

use super::super::RuntimeRenderController;
use super::materials::{critical_scene_materials_ready, SceneMaterialLaunchPlan};
use super::status::LaunchReadiness;

pub(super) fn critical_scene_residency_ready(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    world: &newengine_ecs::World,
    material_plan: Option<&SceneMaterialLaunchPlan>,
) -> LaunchReadiness {
    let parts = [
        critical_static_world_ready(world),
        critical_physics_collision_ready(world),
        critical_primitive_gpu_ready(this, world),
        critical_model_gpu_ready(this, world),
        critical_scene_materials_ready(this, r, world, material_plan),
        critical_terrain_gpu_ready(this, world),
    ];
    LaunchReadiness::aggregate(&parts)
}

fn critical_primitive_gpu_ready(
    this: &RuntimeRenderController,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let mut unique = std::collections::BTreeSet::new();
    for (_entity, primitive) in world.query::<newengine_primitives::Primitive>() {
        unique.insert(primitive.id);
    }
    let total = unique.len() as u32;
    if total == 0 {
        return LaunchReadiness::ready("no primitive gpu meshes declared", 0, 0);
    }
    let resident = unique
        .iter()
        .filter(|id| this.gpu.meshes.prim_cache.contains_key(*id))
        .count() as u32;
    let waiting = total.saturating_sub(resident);
    if waiting == 0 {
        LaunchReadiness::ready(
            format!("primitive gpu meshes resident ready={resident}/{total}"),
            total,
            0,
        )
    } else {
        LaunchReadiness::pending(
            format!(
                "waiting for bounded primitive gpu residency ready={resident}/{total} waiting={waiting}"
            ),
            waiting,
            total,
            0,
        )
    }
}

fn critical_model_gpu_ready(
    this: &RuntimeRenderController,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let sources = world
        .query::<ModelRenderComponent>()
        .filter_map(|(_, model)| {
            let source = model.logical_path.trim();
            (!source.is_empty()).then(|| source.to_owned())
        })
        .collect::<std::collections::BTreeSet<_>>();
    let total = sources.len() as u32;
    if total == 0 {
        return LaunchReadiness::ready("no imported model actors declared", 0, 0);
    }

    let mut resident = 0u32;
    let mut failed = 0u32;
    for source in &sources {
        if this.gpu.meshes.model_bundle_failures.contains_key(source) {
            failed = failed.saturating_add(1);
            continue;
        }
        let Some(bundle) = this.gpu.meshes.model_bundle_cache.get(source) else {
            continue;
        };
        let all_parts_resident = bundle.parts.iter().enumerate().all(|(part_index, _)| {
            let id = RuntimeRenderController::model_part_primitive_id(bundle, part_index);
            this.gpu.meshes.prim_cache.contains_key(&id)
        });
        if all_parts_resident {
            resident = resident.saturating_add(1);
        }
    }
    let waiting = total.saturating_sub(resident).saturating_sub(failed);
    if waiting == 0 {
        LaunchReadiness::ready(
            format!(
                "imported model residency terminal resident={resident}/{total} failed={failed}"
            ),
            total,
            failed,
        )
    } else {
        LaunchReadiness::pending(
            format!(
                "waiting for imported model RenderPrep/GPU residency resident={resident}/{total} waiting={waiting} failed={failed}"
            ),
            waiting,
            total,
            failed,
        )
    }
}

fn critical_static_world_ready(world: &newengine_ecs::World) -> LaunchReadiness {
    let Some(residency) = world.resource::<WorldAssemblyProgress>() else {
        return LaunchReadiness::ready("no incremental static world declared", 0, 0);
    };
    if residency.is_ready() {
        LaunchReadiness::ready(
            format!(
                "static world assembled completed={}/{} failed={}",
                residency.completed(),
                residency.total(),
                residency.failed(),
            ),
            residency.total(),
            residency.failed(),
        )
    } else {
        let waiting = residency.pending().max(residency.failed());
        let reason = if residency.pending() == 0 && residency.failed() > 0 {
            format!(
                "static world assembly failed completed={}/{} failed={} policy='do-not-enter-playable-world'",
                residency.completed(),
                residency.total(),
                residency.failed(),
            )
        } else {
            format!(
                "waiting for incremental static world completed={}/{} pending={} failed={}",
                residency.completed(),
                residency.total(),
                residency.pending(),
                residency.failed(),
            )
        };
        LaunchReadiness::pending(reason, waiting, residency.total(), residency.failed())
    }
}

fn critical_physics_collision_ready(world: &newengine_ecs::World) -> LaunchReadiness {
    let static_total = world
        .query::<crate::gameplay::StaticMeshCollider>()
        .count()
        .min(u32::MAX as usize) as u32;
    if static_total == 0 {
        return LaunchReadiness::ready("no authored static collision declared", 0, 0);
    }

    let Some(progress) = world
        .resource::<crate::gameplay::PhysicsStaticColliderSyncProgress>()
        .copied()
    else {
        return LaunchReadiness::pending(
            format!("waiting for physics collision registration ready=0/{static_total}"),
            static_total,
            static_total,
            0,
        );
    };

    if progress.is_ready() && progress.registered >= static_total {
        LaunchReadiness::ready(
            format!(
                "physics collision resident registered={}/{}",
                progress.registered, static_total
            ),
            static_total,
            progress.failed,
        )
    } else {
        LaunchReadiness::pending(
            format!(
                "registering collision in physics provider ready={}/{} pending={} failed={}",
                progress.registered, static_total, progress.pending, progress.failed,
            ),
            progress
                .pending
                .max(static_total.saturating_sub(progress.registered)),
            static_total,
            progress.failed,
        )
    }
}

fn critical_terrain_gpu_ready(
    this: &RuntimeRenderController,
    world: &newengine_ecs::World,
) -> LaunchReadiness {
    let mut prepared_total = 0_u32;
    let mut resident = 0_u32;
    let mut waiting = 0_u32;
    let mut declared_total = 0_u32;

    for (entity, terrain) in world.query::<ProceduralTerrain>() {
        declared_total = declared_total.saturating_add(1);
        if world.get::<PreparedRenderMesh>(entity).is_none() {
            continue;
        }

        prepared_total = prepared_total.saturating_add(1);
        if this
            .gpu
            .meshes
            .terrain_cache
            .contains_key(&terrain.mesh_key())
        {
            resident = resident.saturating_add(1);
        } else {
            waiting = waiting.saturating_add(1);
        }
    }

    if prepared_total == 0 {
        return if declared_total == 0 {
            LaunchReadiness::ready("no terrain packets declared", 0, 0)
        } else {
            LaunchReadiness::pending(
                format!("waiting for terrain RenderPrep packets declared={declared_total}"),
                declared_total,
                declared_total,
                0,
            )
        };
    }

    let min_ready = crate::runtime_policy::streaming_policy()
        .terrain_launch_min_ready_packets
        .min(prepared_total);

    if resident >= min_ready {
        // Remaining prepared packets are allowed to continue normal post-launch streaming.
        LaunchReadiness {
            ready: true,
            reason: format!(
                "terrain launch packets resident ready={resident}/{prepared_total} declared={declared_total} min_ready={min_ready}"
            ),
            waiting,
            total: prepared_total,
            failed: 0,
        }
    } else {
        LaunchReadiness::pending(
            format!(
                "waiting for first terrain GPU packets resident={resident}/{prepared_total} declared={declared_total} min_ready={min_ready}"
            ),
            waiting,
            prepared_total,
            0,
        )
    }
}
