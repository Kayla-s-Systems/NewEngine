use super::mesh_passes_primitive::{instance_batch_ubo_key, PrimitiveGpuPlan, PrimitivePlanKey};

use newengine_math::{collections::FxHashSet, hash_combine_u64};

use super::scene_mesh_pass::route_diagnostics_due;

use super::*;

#[inline]
fn shadow_light_view_key(light_viewproj: Mat4) -> u64 {
    let mut h = 0xa5ad_50c5_1a57_0001u64;
    for f in light_viewproj.to_cols_array() {
        h = hash_combine_u64(h, f.to_bits() as u64);
    }
    h
}

#[inline]
fn shadow_caster_projected_radius_visible(
    cascade_index: usize,
    cascade_texel_world_size: f32,
    radius_ws: f32,
) -> bool {
    if cascade_index < 2
        || !cascade_texel_world_size.is_finite()
        || cascade_texel_world_size <= 1.0e-6
    {
        return true;
    }
    let projected_radius_texels = radius_ws.abs() / cascade_texel_world_size;
    let min_radius_texels = if cascade_index >= 3 { 0.90 } else { 0.50 };
    projected_radius_texels >= min_radius_texels
}

mod models;
mod primitives;
mod skinned;
mod terrain;

pub use terrain::draw_procedural_terrain_shadow;

pub fn draw_primitives_shadow(
    this: &mut RuntimeRenderController,
    r: &mut dyn newengine_core::render::RenderApi,
    scene: &newengine_scene::Scene,
    lit: newengine_material_domain_api::LitPipeline,
    light_viewproj: Mat4,
    lights: &PackedLights,
    runtime: bool,
    camera_position: Vec3,
    cascade_index: usize,
    cascade_texel_world_size: f32,
) -> newengine_core::EngineResult<()> {
    skinned::draw_skinned_player_primitives_shadow(
        this,
        r,
        scene,
        lit,
        light_viewproj,
        lights,
        runtime,
        camera_position,
        cascade_index,
        cascade_texel_world_size,
    )?;
    models::draw_model_components_shadow(
        this,
        r,
        scene,
        lit,
        light_viewproj,
        lights,
        runtime,
        camera_position,
        cascade_index,
        cascade_texel_world_size,
    )?;
    primitives::draw_primitives_shadow_body(
        this,
        r,
        scene,
        lit,
        light_viewproj,
        lights,
        runtime,
        camera_position,
        cascade_index,
        cascade_texel_world_size,
    )
}

#[cfg(test)]
mod shadow_caster_lod_tests {
    use super::shadow_caster_projected_radius_visible;

    #[test]
    fn shadow_caster_lod_keeps_near_and_rejects_subtexel_distant_casters() {
        assert!(shadow_caster_projected_radius_visible(0, 0.25, 0.05));
        assert!(shadow_caster_projected_radius_visible(1, 0.25, 0.05));
        assert!(!shadow_caster_projected_radius_visible(2, 1.0, 0.40));
        assert!(shadow_caster_projected_radius_visible(2, 1.0, 0.60));
        assert!(!shadow_caster_projected_radius_visible(3, 1.0, 0.80));
        assert!(shadow_caster_projected_radius_visible(3, 1.0, 1.00));
    }

    #[test]
    fn shadow_caster_lod_disables_itself_without_valid_texel_scale() {
        assert!(shadow_caster_projected_radius_visible(3, 0.0, 0.01));
        assert!(shadow_caster_projected_radius_visible(3, f32::NAN, 0.01));
    }
}
