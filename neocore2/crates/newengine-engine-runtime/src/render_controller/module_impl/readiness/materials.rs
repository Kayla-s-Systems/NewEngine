use newengine_core::render::RenderApi;
use newengine_materials::api::MaterialRegistryApi;
use newengine_math::collections::FxHashSet;

use crate::gameplay::TerrainMaterialLayers;
use crate::render_controller::resource_cache::MaterialTextureReadyState;

use super::super::super::material_bindings::LitMaterialPlan;
use super::super::RuntimeRenderController;
use super::status::LaunchReadiness;

const SCENE_TEXTURE_LAUNCH_MIN_RATIO_DEFAULT: f32 = 1.00;
const LAUNCH_OPTIONAL_TEXTURE_TOKENS: &[&str] = &["sky", "skydome", "cloud", "clouds", "moon"];

#[derive(Clone, Debug, Default)]
pub(in crate::render_controller::module_impl) struct SceneMaterialLaunchPlan {
    pub(super) critical_paths: Vec<String>,
    pub(super) optional_paths: Vec<String>,
    pub(super) alpha_critical_paths: FxHashSet<String>,
    pub(super) optional: u32,
}

pub(super) fn build_scene_material_launch_plan(
    world: &newengine_ecs::World,
    mats: &dyn MaterialRegistryApi,
) -> SceneMaterialLaunchPlan {
    let mut unique_paths = FxHashSet::<String>::default();
    let mut alpha_critical_paths = FxHashSet::<String>::default();

    for (_entity, material_ref) in world.query::<newengine_materials::MaterialRef>() {
        let resolved = mats.resolve(material_ref.id);
        let plan = LitMaterialPlan::from_resolved(resolved.as_ref(), [1.0, 1.0, 1.0, 1.0]);
        if plan.alpha_cutoff > 0.0 {
            if let Some(path) = plan.base_color_texture {
                alpha_critical_paths.insert(path.to_owned());
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
    alpha_critical_paths.retain(|path| !is_launch_gate_optional_texture(path));
    critical_paths.sort_unstable_by(|a, b| {
        let a_alpha = alpha_critical_paths.contains(a);
        let b_alpha = alpha_critical_paths.contains(b);
        b_alpha.cmp(&a_alpha).then_with(|| a.cmp(b))
    });

    SceneMaterialLaunchPlan {
        critical_paths,
        optional_paths,
        alpha_critical_paths,
        optional,
    }
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
    let plan = material_plan
        .or(owned_plan.as_ref())
        .expect("scene launch material plan");

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
    let mut alpha_waiting = 0_u32;
    let mut alpha_failed = 0_u32;

    for path in &plan.critical_paths {
        this.request_material_texture(path);
        let alpha_critical = plan.alpha_critical_paths.contains(path);
        match this.material_texture_ready_state(r, path, "render.launch_gate") {
            MaterialTextureReadyState::Ready(_) => {}
            MaterialTextureReadyState::Failed => {
                failed = failed.saturating_add(1);
                if alpha_critical {
                    alpha_failed = alpha_failed.saturating_add(1);
                }
            }
            MaterialTextureReadyState::Waiting => {
                waiting = waiting.saturating_add(1);
                if alpha_critical {
                    alpha_waiting = alpha_waiting.saturating_add(1);
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
    let configured_min_ready = crate::env_config::var_u32(
        "NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY",
        total,
        0,
        total.max(1),
    )
    .min(total);
    let visual_floor = scene_texture_launch_visual_floor(total);
    let min_ready = configured_min_ready.max(visual_floor).min(total);

    if alpha_waiting > 0 || alpha_failed > 0 {
        let alpha_total = plan.alpha_critical_paths.len() as u32;
        LaunchReadiness::pending(
            format!(
                "waiting for alpha-critical texture residency ready={}/{} waiting={} failed={} policy='Masked base textures never use opaque fallback'",
                alpha_total.saturating_sub(alpha_waiting).saturating_sub(alpha_failed),
                alpha_total,
                alpha_waiting,
                alpha_failed,
            ),
            waiting,
            total,
            failed,
        )
    } else if waiting == 0 {
        LaunchReadiness::ready(
            if failed == 0 {
                format!("scene material textures ready total={total}")
            } else {
                format!(
                    "scene material textures ready with fallbacks total={total} failed={failed}"
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
                "scene material textures partially resident ready={ready_count}/{total} waiting={waiting} failed={failed} min_ready={min_ready} policy='NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_READY/NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO'"
            ),
            waiting,
            total,
            failed,
        }
    } else {
        LaunchReadiness::pending(
            format!(
                "waiting for scene texture residency ready={ready_count}/{total} waiting={waiting} failed={failed} min_ready={min_ready}"
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
    let ratio = crate::env_config::var_f32(
        "NEWENGINE_SCENE_TEXTURE_LAUNCH_MIN_RATIO",
        SCENE_TEXTURE_LAUNCH_MIN_RATIO_DEFAULT,
        0.50,
        1.00,
    );
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
