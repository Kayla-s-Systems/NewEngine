use super::*;

#[test]
fn pack_exposes_expected_draw_providers() {
    let pack = StandardRenderFeaturePack::new();
    let ids = pack
        .draw_list_providers()
        .into_iter()
        .map(|provider| provider.id())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            STANDARD_TERRAIN_PROVIDER_ID,
            STANDARD_PRIMITIVE_MESH_PROVIDER_ID,
        ]
    );
}

#[test]
fn pack_exposes_expected_light_providers() {
    let pack = StandardRenderFeaturePack::new();
    let ids = pack
        .light_extraction_providers()
        .into_iter()
        .map(|provider| provider.id())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            STANDARD_DIRECTIONAL_SHADOW_PROVIDER_ID,
            STANDARD_POINT_CUBE_SHADOW_PROVIDER_ID,
            STANDARD_SPOT_SHADOW_PROVIDER_ID,
            STANDARD_AMBIENT_OCCLUSION_PROVIDER_ID,
        ]
    );
}

#[test]
fn pack_uses_standard_lit_pipeline() {
    let pack = StandardRenderFeaturePack::new();
    assert_eq!(
        pack.primary_lit_material_domain(),
        newengine_material_domain_standard::STANDARD_LIT_PIPELINE_KEY
    );
}
