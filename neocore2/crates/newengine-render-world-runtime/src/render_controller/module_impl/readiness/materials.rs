use newengine_core::render::RenderApi;
use newengine_materials::api::MaterialRegistryApi;
use newengine_math::collections::FxHashSet;

use crate::render_controller::resource_cache::MaterialTextureReadyState;
use newengine_gameplay_world_runtime::gameplay::TerrainMaterialLayers;

use super::super::super::material_bindings::LitMaterialPlan;
use super::super::RuntimeRenderController;
use super::status::LaunchReadiness;

const LAUNCH_OPTIONAL_TEXTURE_TOKENS: &[&str] = &["sky", "skydome", "cloud", "clouds", "moon"];

#[derive(Clone, Debug, Default)]
pub(in crate::render_controller::module_impl) struct SceneMaterialLaunchPlan {
    pub(super) critical_paths: Vec<String>,
    pub(super) optional_paths: Vec<String>,
    pub(super) fallback_forbidden_paths: FxHashSet<String>,
    pub(super) optional: u32,
}

#[derive(Clone, Debug, Default)]
struct SceneMaterialLaunchPlanCache {
    observed_world_tick: u64,
    material_revision: u64,
    rebuild_count: u64,
    plan: SceneMaterialLaunchPlan,
}

/// Returns the launch material plan derived from the current world/material
/// revisions. The plan used to rescan every MaterialRef/TerrainMaterialLayers on
/// each loading frame even when only GPU/upload readiness changed.
pub(super) fn cached_scene_material_launch_plan(
    world: &mut newengine_ecs::World,
    mats: &dyn MaterialRegistryApi,
) -> SceneMaterialLaunchPlan {
    let current_tick = world.tick();
    let material_revision = mats.revision();
    let (observed_tick, observed_material_revision, cached_plan, rebuild_count) = world
        .resource::<SceneMaterialLaunchPlanCache>()
        .map(|cache| {
            (
                cache.observed_world_tick,
                cache.material_revision,
                cache.plan.clone(),
                cache.rebuild_count,
            )
        })
        .unwrap_or_default();

    let world_dirty = observed_tick == 0
        || world.entities_changed_since(observed_tick)
        || world.any_changed_since::<newengine_materials::MaterialRef>(observed_tick)
        || world.any_added_since::<newengine_materials::MaterialRef>(observed_tick)
        || world.any_changed_since::<TerrainMaterialLayers>(observed_tick)
        || world.any_added_since::<TerrainMaterialLayers>(observed_tick);
    let material_dirty = observed_material_revision != material_revision;

    let (plan, rebuild_count) = if world_dirty || material_dirty {
        let plan = build_scene_material_launch_plan(world, mats);
        let rebuild_count = rebuild_count.saturating_add(1);
        if rebuild_count <= 4 || rebuild_count.is_multiple_of(32) {
            newengine_ulog_api::ulog::debug!(
                "render launch material plan: rebuilt world_tick={} previous_tick={} material_revision={} previous_material_revision={} critical={} optional={} rebuild_count={} policy='revision-driven'",
                current_tick,
                observed_tick,
                material_revision,
                observed_material_revision,
                plan.critical_paths.len(),
                plan.optional_paths.len(),
                rebuild_count,
            );
        }
        (plan, rebuild_count)
    } else {
        (cached_plan, rebuild_count)
    };

    world.insert_resource(SceneMaterialLaunchPlanCache {
        observed_world_tick: current_tick,
        material_revision,
        rebuild_count,
        plan: plan.clone(),
    });
    plan
}

pub(super) fn build_scene_material_launch_plan(
    world: &newengine_ecs::World,
    mats: &dyn MaterialRegistryApi,
) -> SceneMaterialLaunchPlan {
    let mut unique_paths = FxHashSet::<String>::default();
    let mut fallback_forbidden_paths = FxHashSet::<String>::default();

    for (_entity, material_ref) in world.query::<newengine_materials::MaterialRef>() {
        let resolved = mats.resolve(material_ref.id);
        let plan = LitMaterialPlan::from_resolved(resolved.as_ref(), [1.0, 1.0, 1.0, 1.0]);
        if plan.alpha_cutoff > 0.0 {
            if let Some(path) = plan.base_color_texture {
                fallback_forbidden_paths.insert(path.to_owned());
            }
        }
        let player_skin = world
            .get::<newengine_gameplay_world_runtime::gameplay::PlayerSkinBinding>(_entity)
            .is_some();
        let equipped_weapon = world
            .get::<newengine_gameplay_world_runtime::gameplay::PlayerVisualPart>(_entity)
            .is_some_and(|part| {
                part.kind
                    == newengine_gameplay_world_runtime::gameplay::PlayerVisualKind::EquippedWeapon
            });
        if player_skin {
            if let Some(path) = plan.base_color_texture {
                // Character albedo must never be represented by the generic white fallback.
                fallback_forbidden_paths.insert(path.to_owned());
            }
        }
        if equipped_weapon {
            // Equipped solid objects are admitted as a complete authored PBR surface. Avoid a
            // one-frame white albedo, flat normal, or white roughness stage while late bindings
            // stream after inventory/equipment creation.
            for path in [
                plan.base_color_texture,
                plan.normal_texture,
                plan.roughness_texture,
            ]
            .into_iter()
            .flatten()
            {
                fallback_forbidden_paths.insert(path.to_owned());
            }
        }
        for path in [
            plan.base_color_texture,
            plan.normal_texture,
            plan.roughness_texture,
        ]
        .into_iter()
        .flatten()
        {
            unique_paths.insert(path.to_owned());
        }
    }

    for (_entity, layers) in world.query::<TerrainMaterialLayers>() {
        unique_paths.insert(layers.forest_base_texture.clone());
        unique_paths.insert(layers.sand_base_texture.clone());
        unique_paths.insert(layers.rock_base_texture.clone());
    }

    let mut optional = 0_u32;
    let mut critical_paths = Vec::with_capacity(unique_paths.len());
    let mut optional_paths = Vec::new();
    for path in unique_paths {
        if is_launch_gate_optional_texture(&path) {
            optional = optional.saturating_add(1);
            optional_paths.push(path);
        } else {
            critical_paths.push(path);
        }
    }
    optional_paths.sort_unstable();
    fallback_forbidden_paths.retain(|path| !is_launch_gate_optional_texture(path));
    critical_paths.sort_unstable_by(|a, b| {
        let a_hard = fallback_forbidden_paths.contains(a);
        let b_hard = fallback_forbidden_paths.contains(b);
        b_hard.cmp(&a_hard).then_with(|| a.cmp(b))
    });

    SceneMaterialLaunchPlan {
        critical_paths,
        optional_paths,
        fallback_forbidden_paths,
        optional,
    }
}

fn extend_launch_plan_with_model_materials(
    this: &RuntimeRenderController,
    world: &newengine_ecs::World,
    plan: &mut SceneMaterialLaunchPlan,
) {
    let mut critical = plan
        .critical_paths
        .iter()
        .cloned()
        .collect::<FxHashSet<_>>();
    let mut optional_paths = plan
        .optional_paths
        .iter()
        .cloned()
        .collect::<FxHashSet<_>>();

    for (_entity, model) in
        world.query::<newengine_gameplay_world_runtime::gameplay::ModelRenderComponent>()
    {
        let Some(bundle) = this
            .gpu
            .meshes
            .model_bundle_cache
            .get(model.logical_path.trim())
        else {
            continue;
        };
        for part in &bundle.parts {
            let resolved = newengine_materials::MaterialResolved {
                id: newengine_materials::MaterialId::invalid(),
                desc: part.material.descriptor,
                textures: part.material.textures.clone(),
            };
            let material =
                LitMaterialPlan::from_resolved(Some(&resolved), part.material.fallback_color);
            if material.alpha_cutoff > 0.0 {
                if let Some(path) = material.base_color_texture {
                    if !is_launch_gate_optional_texture(path) {
                        plan.fallback_forbidden_paths.insert(path.to_owned());
                    }
                }
            }
            for path in [
                material.base_color_texture,
                material.normal_texture,
                material.roughness_texture,
            ]
            .into_iter()
            .flatten()
            {
                if is_launch_gate_optional_texture(path) {
                    optional_paths.insert(path.to_owned());
                } else {
                    critical.insert(path.to_owned());
                }
            }
        }
    }

    plan.critical_paths = critical.into_iter().collect();
    plan.optional_paths = optional_paths.into_iter().collect();
    plan.optional = plan.optional_paths.len() as u32;
    plan.optional_paths.sort_unstable();
    plan.fallback_forbidden_paths
        .retain(|path| !is_launch_gate_optional_texture(path));
    plan.critical_paths.sort_unstable_by(|a, b| {
        let a_hard = plan.fallback_forbidden_paths.contains(a);
        let b_hard = plan.fallback_forbidden_paths.contains(b);
        b_hard.cmp(&a_hard).then_with(|| a.cmp(b))
    });
}

pub(super) fn critical_scene_materials_ready(
    this: &mut RuntimeRenderController,
    r: &mut dyn RenderApi,
    world: &newengine_ecs::World,
    material_plan: Option<&SceneMaterialLaunchPlan>,
) -> LaunchReadiness {
    let owned_plan = material_plan.is_none().then(|| {
        let mats_lock = this.bridges.scene.materials();
        let mats = mats_lock.read();
        build_scene_material_launch_plan(world, &*mats)
    });
    let mut merged_plan = material_plan
        .cloned()
        .or(owned_plan)
        .expect("scene launch material plan");
    extend_launch_plan_with_model_materials(this, world, &mut merged_plan);
    let plan = &merged_plan;

    // Optional environment textures must not block launch, but they should still
    // start decoding while the loading gate is active. Previously sky/cloud textures
    // were first requested by the first public world frame, causing worker contention
    // and upload bursts exactly at handoff.
    for path in &plan.optional_paths {
        this.request_material_texture(path);
    }

    let total = plan.critical_paths.len() as u32;
    let mut waiting = 0_u32;
    let mut failed = 0_u32;
    let mut failed_paths = Vec::<String>::new();
    let mut fallback_forbidden_waiting = 0_u32;
    let mut fallback_forbidden_failed = 0_u32;

    for path in &plan.critical_paths {
        this.request_material_texture(path);
        let fallback_forbidden = plan.fallback_forbidden_paths.contains(path);
        match this.material_texture_ready_state(r, path, "render.launch_gate") {
            MaterialTextureReadyState::Ready(_) => {}
            MaterialTextureReadyState::Failed => {
                failed = failed.saturating_add(1);
                failed_paths.push(path.clone());
                if fallback_forbidden {
                    fallback_forbidden_failed = fallback_forbidden_failed.saturating_add(1);
                }
                if this.frame.frame_index <= 4 || this.frame.frame_index.is_multiple_of(120) {
                    newengine_ulog_api::ulog::warn!(
                        "render launch texture failed path='{}' fallback_forbidden={} policy='failed material texture must remain diagnosable'",
                        path,
                        fallback_forbidden,
                    );
                }
            }
            MaterialTextureReadyState::Waiting => {
                waiting = waiting.saturating_add(1);
                if fallback_forbidden {
                    fallback_forbidden_waiting = fallback_forbidden_waiting.saturating_add(1);
                }
            }
        }
    }

    if total == 0 {
        return LaunchReadiness::ready(
            if plan.optional == 0 {
                "no critical scene textures declared".to_owned()
            } else {
                format!(
                    "only optional environment textures declared optional={}",
                    plan.optional
                )
            },
            0,
            0,
        );
    }

    let ready_count = total.saturating_sub(waiting).saturating_sub(failed);
    let configured_min_ready = newengine_runtime_policy::streaming_policy()
        .scene_texture_launch_min_ready
        .unwrap_or(total)
        .min(total);
    let visual_floor = scene_texture_launch_visual_floor(total);
    let min_ready = configured_min_ready.max(visual_floor).min(total);

    if fallback_forbidden_waiting > 0 {
        let fallback_forbidden_total = plan.fallback_forbidden_paths.len() as u32;
        LaunchReadiness::pending(
            format!(
                "waiting for fallback-forbidden texture residency ready={}/{} waiting={} failed={} policy='masked, skinned-character, and equipped-weapon base textures never use generic white fallback'",
                fallback_forbidden_total.saturating_sub(fallback_forbidden_waiting).saturating_sub(fallback_forbidden_failed),
                fallback_forbidden_total,
                fallback_forbidden_waiting,
                fallback_forbidden_failed,
            ),
            waiting,
            total,
            failed,
        )
    } else if fallback_forbidden_failed > 0 {
        // A permanent decode/upload failure is not pending work. Keeping the launch gate
        // closed here only spins until the global soft timeout even though waiting==0.
        // Preserve the strict visual contract by leaving failed textures unavailable
        // (never substitute generic white), but let the runtime enter Play in a degraded,
        // diagnosable state so the OS/event loop is not held hostage by an impossible wait.
        let fallback_forbidden_total = plan.fallback_forbidden_paths.len() as u32;
        LaunchReadiness::ready(
            format!(
                "fallback-forbidden texture loads failed permanently ready={}/{} failed={} paths={failed_paths:?} policy='no generic white fallback; affected visuals remain unavailable/diagnosable; no impossible launch wait'",
                fallback_forbidden_total.saturating_sub(fallback_forbidden_failed),
                fallback_forbidden_total,
                fallback_forbidden_failed,
            ),
            total,
            failed,
        )
    } else if waiting == 0 {
        LaunchReadiness::ready(
            if failed == 0 {
                format!("scene material textures ready total={total}")
            } else {
                format!(
                    "scene material textures ready with fallbacks total={total} failed={failed} paths={failed_paths:?}"
                )
            },
            total,
            failed,
        )
    } else if min_ready > 0 && ready_count >= min_ready {
        // The launch gate may release with non-critical textures still streaming. Preserve the
        // waiting count for diagnostics even though this subsystem is logically ready.
        LaunchReadiness {
            ready: true,
            reason: format!(
                "scene material textures partially resident ready={ready_count}/{total} waiting={waiting} failed={failed} paths={failed_paths:?} min_ready={min_ready} policy='NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY/NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO'"
            ),
            waiting,
            total,
            failed,
        }
    } else {
        LaunchReadiness::pending(
            format!(
                "waiting for scene texture residency ready={ready_count}/{total} waiting={waiting} failed={failed} paths={failed_paths:?} min_ready={min_ready}"
            ),
            waiting,
            total,
            failed,
        )
    }
}

#[inline]
fn is_launch_gate_optional_texture(path: &str) -> bool {
    path.split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .any(|token| {
            LAUNCH_OPTIONAL_TEXTURE_TOKENS
                .iter()
                .any(|candidate| token == *candidate)
        })
}

fn scene_texture_launch_visual_floor(total: u32) -> u32 {
    if total <= 1 {
        return total;
    }
    let ratio = newengine_runtime_policy::streaming_policy().scene_texture_launch_min_ratio;
    ((total as f32) * ratio).ceil() as u32
}

#[cfg(test)]
mod tests {
    use super::is_launch_gate_optional_texture;

    #[test]
    fn optional_texture_classification_uses_tokens_not_substrings() {
        assert!(is_launch_gate_optional_texture(
            "textures/environment/sky_clouds_v2.ytd@clouds"
        ));
        assert!(is_launch_gate_optional_texture(
            "textures/environment/moon.ytd@moon"
        ));
        assert!(!is_launch_gate_optional_texture(
            "textures/props/whiskey_barrel.ytd@base_color"
        ));
    }
}
