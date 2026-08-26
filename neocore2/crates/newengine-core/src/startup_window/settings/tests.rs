#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_independent_aa_and_expensive_effects() {
        let mut settings = StartupLaunchSettings::default();
        settings.graphics.preset = GraphicsPreset::Custom;
        settings.graphics.msaa_samples = 8;
        settings.graphics.fxaa_enabled = true;
        settings.graphics.taa_enabled = true;
        settings.graphics.ssao_enabled = true;
        settings.display.render_scale = 4.0;
        settings.normalize();

        assert_eq!(settings.graphics.msaa_samples, 8);
        assert!(settings.graphics.fxaa_enabled);
        assert!(settings.graphics.taa_enabled);
        assert!(settings.graphics.ssao_enabled);
        assert_eq!(settings.display.render_scale, 2.0);
    }

    #[test]
    fn disabled_shadows_force_off_quality() {
        let mut settings = StartupLaunchSettings::default();
        settings.graphics.shadows_enabled = false;
        settings.graphics.shadow_quality = ShadowQuality::Cinematic;
        settings.normalize();
        assert_eq!(settings.graphics.shadow_quality, ShadowQuality::Off);
    }

    #[test]
    fn normalizes_lod_and_shadow_overrides_without_forcing_scene_defaults() {
        let mut settings = StartupLaunchSettings::default();
        assert_eq!(settings.graphics.shadow_cascade_count, 0);
        assert_eq!(settings.graphics.shadow_map_resolution, 0);
        settings.graphics.lod_distance_scale = 9.0;
        settings.graphics.shadow_cascade_count = 99;
        settings.graphics.shadow_map_resolution = 3000;
        settings.normalize();
        assert_eq!(settings.graphics.lod_distance_scale, 2.0);
        assert_eq!(settings.graphics.shadow_cascade_count, 4);
        assert_eq!(settings.graphics.shadow_map_resolution, 4096);

        settings.graphics.shadow_map_resolution = 16284;
        settings.normalize();
        assert_eq!(settings.graphics.shadow_map_resolution, 16284);
    }

    #[test]
    fn preset_is_only_a_starting_point_and_controls_remain_independent() {
        let mut graphics = StartupGraphicsSettings::default();
        graphics.apply_preset(GraphicsPreset::High);
        graphics.fxaa_enabled = false;
        graphics.msaa_samples = 8;
        graphics.mark_custom();
        graphics.normalize();

        assert_eq!(graphics.preset, GraphicsPreset::Custom);
        assert_eq!(graphics.msaa_samples, 8);
        assert!(!graphics.fxaa_enabled);
        assert!(graphics.ssao_enabled);
    }
}
