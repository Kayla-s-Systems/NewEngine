#[cfg(test)]
mod game_data_lighting_tests {
    use super::*;

    #[test]
    fn project_game_data_is_authoritative_for_shadow_quality() {
        let mut data = newengine_game_data::default_game_data().clone();
        data.world.lighting.sun_intensity = 3.25;
        data.world.shadows.cascade_count = 1;
        data.world.shadows.max_distance = 96.0;
        data.world.shadows.filter = "pcf".to_owned();
        data.world.day_night.time_of_day_hours = 11.5;

        let spec = game_data_lighting_spec(&data);
        assert_eq!(spec.shadows.cascade_count, 1);
        assert_eq!(spec.shadows.max_distance, 96.0);
        assert_eq!(spec.shadows.filter, newengine_lighting::ShadowFilter::Pcf);
        assert_eq!(spec.sun_intensity, 3.25);
        assert_eq!(spec.day_night.time_of_day_hours, 11.5);
    }

    #[test]
    fn empty_game_data_sky_asset_fields_preserve_ytyp_resolved_assets() {
        let mut data = newengine_game_data::default_game_data().clone();
        data.world.sky.definition_ref =
            "shared/definitions/environment/default_sky.ytyp@default_sky".to_owned();
        data.world.sky.mesh.clear();
        data.world.sky.cloud_dictionary.clear();
        data.world.sky.moon_texture.clear();
        data.world.sky.cloud_profile = "cloudless".to_owned();

        let fallback = AuthoredFpsSkySpec {
            definition_ref: data.world.sky.definition_ref.clone(),
            render_options: newengine_model_domain_api::MeshRenderOptions::sky_background(),
            radius: 220.0,
            mesh: "models/environment/skydome.ydd@skydome_high".to_owned(),
            follow_camera: true,
            environment_profile: "environment.default".to_owned(),
            environment_region: String::new(),
            environment_biome: String::new(),
            cloud_dictionary: "textures/environment/sky_clouds_v2.ytd".to_owned(),
            cloud_profile: "temperate_cumulus_dynamic".to_owned(),
            sun_radius: 18.0,
            moon_radius: 13.5,
            moon_texture: "textures/environment/skydome.ytd@moon_new".to_owned(),
            atmosphere: AuthoredFpsSkyAtmosphereSpec {
                day_zenith: [0.30, 0.55, 0.96],
                day_horizon: [0.72, 0.86, 1.0],
                dusk_zenith: [0.16, 0.20, 0.40],
                dusk_horizon: [1.0, 0.47, 0.20],
                night_zenith: [0.006, 0.010, 0.030],
                night_horizon: [0.020, 0.024, 0.052],
                cloud_day: [0.96, 0.98, 1.0],
                cloud_night: [0.04, 0.05, 0.085],
                night_sky_strength: 0.35,
                cloud_coverage: 0.0,
                cloud_softness: 0.56,
            },
        };

        let merged = game_data_sky_spec(&data, &fallback);
        assert_eq!(merged.mesh, fallback.mesh);
        assert_eq!(merged.cloud_dictionary, fallback.cloud_dictionary);
        assert_eq!(merged.moon_texture, fallback.moon_texture);
        assert_eq!(merged.cloud_profile, "cloudless");
    }

    #[test]
    fn project_game_data_lighting_is_sanitized_before_runtime_install() {
        let mut data = newengine_game_data::default_game_data().clone();
        data.world.shadows.cascade_count = 99;
        data.world.shadows.max_distance = 50_000.0;
        data.world.shadows.filter = "unknown".to_owned();
        data.world.day_night.day_of_year = 999;
        data.world.day_night.latitude_degrees = 120.0;

        let spec = game_data_lighting_spec(&data);
        assert_eq!(spec.shadows.cascade_count, 4);
        assert_eq!(spec.shadows.max_distance, 1000.0);
        assert_eq!(spec.shadows.filter, newengine_lighting::ShadowFilter::Pcf);
        assert_eq!(spec.day_night.day_of_year, 366);
        assert_eq!(spec.day_night.latitude_degrees, 89.0);
    }
}
