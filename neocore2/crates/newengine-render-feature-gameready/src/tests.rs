use super::*;

#[test]
fn pack_exposes_expected_draw_providers() {
    let pack = GameReadyRenderFeaturePack::new();
    let ids = pack
        .draw_list_providers()
        .into_iter()
        .map(|provider| provider.id())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            GAME_READY_TERRAIN_PROVIDER_ID,
            GAME_READY_PRIMITIVE_MESH_PROVIDER_ID,
        ]
    );
}

#[test]
fn pack_exposes_expected_light_providers() {
    let pack = GameReadyRenderFeaturePack::new();
    let ids = pack
        .light_extraction_providers()
        .into_iter()
        .map(|provider| provider.id())
        .collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            GAME_READY_DIRECTIONAL_SHADOW_PROVIDER_ID,
            GAME_READY_POINT_CUBE_SHADOW_PROVIDER_ID,
            GAME_READY_SPOT_SHADOW_PROVIDER_ID,
            GAME_READY_AMBIENT_OCCLUSION_PROVIDER_ID,
        ]
    );
}

#[test]
fn pack_uses_game_ready_lit_pipeline() {
    let pack = GameReadyRenderFeaturePack::new();
    assert_eq!(
        pack.primary_lit_material_domain(),
        newengine_material_domain_gameready::GAME_READY_LIT_PIPELINE_KEY
    );
}
