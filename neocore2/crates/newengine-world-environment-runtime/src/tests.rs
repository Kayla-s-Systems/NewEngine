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

#[test]
fn cloud_regimes_are_bounded_and_non_cloud_weather_does_not_fake_overcast() {
    use crate::profile_catalog::pattern_by_id;

    let clear = pattern_by_id("weather.clear.dry_high_pressure");
    let fair = pattern_by_id("weather.cloudy.fair_cumulus");
    let overcast = pattern_by_id("weather.overcast.stratus_deck");
    let fog = pattern_by_id("weather.fog.ground_radiation");
    let dust = pattern_by_id("weather.dust_storm.front");

    for pattern in [clear, fair, overcast, fog, dust] {
        assert!(pattern.cloud_floor <= pattern.cloud_ceiling);
        assert!((0.0..=1.0).contains(&pattern.cloud_floor));
        assert!((0.0..=1.0).contains(&pattern.cloud_ceiling));
    }
    assert!(clear.cloud_ceiling <= 0.32);
    assert!(fair.cloud_floor < 0.35 && fair.cloud_ceiling < 0.70);
    assert!(overcast.cloud_floor >= 0.65);
    assert!(
        fog.cloud_ceiling <= 0.30,
        "ground fog must not imply an overcast deck"
    );
    assert!(
        dust.cloud_ceiling <= 0.22,
        "dust aerosol must not imply an overcast deck"
    );
}

#[test]
fn forest_road_weather_has_real_clear_sky_windows() {
    let mut clear_frames = 0usize;
    let mut fair_frames = 0usize;
    let mut overcast_frames = 0usize;
    let mut minimum_clear_coverage = 1.0_f32;

    for seed in 0..128_u64 {
        for normalized_day in [0.25_f64, 8.65_f64 / 24.0, 0.50, 0.75] {
            let mut req = EnvironmentFrameRequest::default();
            req.environment_profile.profile_id = "environment.game_ready_forest_road".to_owned();
            req.seed = seed;
            req.time.game.day_index = 171;
            req.time.game.normalized_day = normalized_day;
            let frame = build_default_environment_frame(
                "environment.default",
                WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
                req,
            );
            match frame.global.active_weather_profile.as_str() {
                "weather.clear.dry_high_pressure" => {
                    clear_frames += 1;
                    minimum_clear_coverage = minimum_clear_coverage.min(frame.clouds.coverage);
                    assert!(
                        frame.clouds.coverage <= 0.301,
                        "clear regime escaped its cloud ceiling coverage={}",
                        frame.clouds.coverage
                    );
                    if frame.clouds.coverage < 0.12 {
                        assert!(
                            frame
                                .clouds
                                .layers
                                .iter()
                                .all(|layer| layer.coverage < 0.12),
                            "sparse clear sky retained a broad cloud layer: {:?}",
                            frame.clouds.layers
                        );
                        assert!(
                            frame.clouds.layers.iter().all(|layer| layer.density < 0.10),
                            "sparse clear sky retained optically dense cloud layers: {:?}",
                            frame.clouds.layers
                        );
                    }
                }
                "weather.cloudy.fair_cumulus" => {
                    fair_frames += 1;
                    assert!((0.259..=0.641).contains(&frame.clouds.coverage));
                }
                "weather.overcast.stratus_deck" => {
                    overcast_frames += 1;
                    assert!(frame.clouds.coverage >= 0.679);
                }
                other => panic!("unexpected ForestRoad weather pattern {other}"),
            }
        }
    }

    assert!(
        clear_frames > fair_frames,
        "clear={clear_frames} fair={fair_frames}"
    );
    assert!(
        fair_frames > 100,
        "fair cumulus disappeared from weather variation"
    );
    assert!(overcast_frames > 0, "overcast became unreachable");
    assert!(
        minimum_clear_coverage < 0.12,
        "clear weather never produces a genuinely sparse sky min={minimum_clear_coverage}"
    );
}
