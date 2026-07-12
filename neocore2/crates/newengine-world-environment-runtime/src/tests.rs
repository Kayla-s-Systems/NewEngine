use newengine_world_environment_api::{EnvironmentFrameRequest, Vec3Dto};

use crate::{
    constants::{WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE},
    default_provider::build_default_environment_frame,
    provider_state::EnvironmentProviderState,
};

#[test]
fn default_provider_is_deterministic_for_same_request() {
    let mut req = EnvironmentFrameRequest {
        frame_id: 17,
        seed: 42,
        ..EnvironmentFrameRequest::default()
    };
    req.time.game.normalized_day = 0.5;
    let a = build_default_environment_frame(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req.clone(),
    );
    let b = build_default_environment_frame(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req,
    );
    assert_eq!(
        a.diagnostics.deterministic_key,
        b.diagnostics.deterministic_key
    );
    assert_vec3_nearly_eq(
        a.celestial.sun.direction_world,
        b.celestial.sun.direction_world,
    );
    assert_eq!(a.weather.state, b.weather.state);
    assert_eq!(a.environment_objects, b.environment_objects);
    assert_eq!(
        a.visual_assets.visual_group_id,
        "environment.visuals.game_ready_skydome.v1"
    );
    assert_eq!(
        a.visual_assets.texture_dictionary_ref,
        "textures/environment/skydome.ytd"
    );
    assert_eq!(
        a.visual_assets.sky_texture_ref,
        "textures/environment/skydome.ytd@starfield"
    );
    assert_eq!(
        a.visual_assets.cloud_density_texture_ref,
        "textures/environment/sky_clouds_v2.ytd@cloud_base_shape"
    );
    assert_eq!(
        a.visual_assets.cloud_detail_texture_ref,
        "textures/environment/sky_clouds_v2.ytd@cloud_detail_erosion"
    );
    assert_eq!(
        a.visual_assets.moon_disk_texture_ref,
        "textures/environment/skydome.ytd@moon_new"
    );
    assert!(!a
        .visual_assets
        .sun_disk_texture_ref
        .contains("textures/sky/celestial.ytd"));
    assert_eq!(
        a.visual_assets.sky_texture_ref,
        a.consumer_packets.render.sky_texture_ref
    );
    assert_eq!(
        a.visual_assets.visual_group_id,
        a.consumer_packets.render.visual_group_id
    );
    assert!(
        !a.consumer_packets.streaming.residency_intents.is_empty() || a.clouds.coverage <= 0.20
    );
    assert!(!a.diagnostics.degraded);
}

#[test]
fn forest_road_profile_uses_temperate_non_storm_baseline() {
    for seed in 0..32u64 {
        let mut req = EnvironmentFrameRequest::default();
        req.environment_profile.profile_id = "environment.game_ready_forest_road".to_owned();
        req.active_region = Some("game_ready.forest_road".to_owned());
        req.active_biome = Some("temperate_forest".to_owned());
        req.seed = seed;
        req.time.game.day_index = 171;
        req.time.game.normalized_day = 8.65 / 24.0;
        let frame = build_default_environment_frame(
            "environment.default",
            WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
            req,
        );
        assert_eq!(
            frame.global.active_environment_profile,
            "environment.game_ready_forest_road"
        );
        assert_eq!(
            frame.global.weather_table_ref,
            "weather/game_ready_forest_road.yweather@table"
        );
        assert!(
            !frame.global.active_weather_profile.contains("rain")
                && !frame.global.active_weather_profile.contains("storm"),
            "seed={seed} selected {}",
            frame.global.active_weather_profile
        );
        assert!(frame.celestial.sun.direction_world.y > 0.0);
        assert!(frame.lighting_intent.sky_light_intensity >= 0.02);
    }
}

#[test]
fn profile_selection_is_exact_descriptor_not_substring_weather_force() {
    let mut req = EnvironmentFrameRequest::default();
    req.environment_profile.profile_id =
        "environment.fake_storm_name_that_is_not_registered".to_owned();
    req.seed = 7;
    req.time.game.normalized_day = 0.50;
    let frame = build_default_environment_frame(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        req,
    );
    assert_eq!(
        frame.global.active_environment_profile,
        "environment.default"
    );
    assert!(frame
        .diagnostics
        .warnings
        .iter()
        .any(|warning| warning.contains("unknown environment profile")));
    assert!(frame
        .diagnostics
        .reasons
        .iter()
        .any(|reason| reason.contains("weather_table=")));
}

#[test]
fn visual_asset_refs_use_existing_grouped_skydome_dictionary() {
    let frame = build_default_environment_frame(
        "environment.default",
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
        EnvironmentFrameRequest::default(),
    );
    let serialized = serde_json::to_string(&frame.visual_assets).expect("visual assets serialize");
    assert!(serialized.contains("textures/environment/skydome.ytd"));
    assert!(!serialized.contains("textures/sky/highlands_sky.ytd"));
    assert!(!serialized.contains("textures/sky/default_sky.ytd"));
    assert!(!serialized.contains("textures/sky/alpine_sky.ytd"));
    assert!(!serialized.contains("textures/sky/desert_sky.ytd"));
    assert!(!serialized.contains("textures/sky/celestial.ytd"));
}

fn assert_vec3_nearly_eq(left: Vec3Dto, right: Vec3Dto) {
    const EPSILON: f32 = 0.000_001;
    assert!(
        (left.x - right.x).abs() <= EPSILON,
        "x differs: left={left:?} right={right:?}"
    );
    assert!(
        (left.y - right.y).abs() <= EPSILON,
        "y differs: left={left:?} right={right:?}"
    );
    assert!(
        (left.z - right.z).abs() <= EPSILON,
        "z differs: left={left:?} right={right:?}"
    );
}

#[test]
fn null_provider_returns_visible_degraded_frame() {
    let state = EnvironmentProviderState::new(
        "environment.null",
        WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE,
        true,
    );
    assert!(state.last_frame.diagnostics.degraded);
    assert_eq!(
        state.last_frame.diagnostics.provider_route,
        WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE
    );
}
