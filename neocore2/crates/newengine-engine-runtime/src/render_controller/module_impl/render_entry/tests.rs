#[cfg(test)]
mod runtime_debug_overlay_setting_tests {
    use super::{
        parse_runtime_debug_overlay_setting, resolve_viewport_clear_color,
        ASSET_PREVIEW_EDITOR_CLEAR_COLOR,
    };

    #[test]
    fn external_preview_uses_editor_gray_clear_color() {
        let resolved =
            resolve_viewport_clear_color(true, Some([0.0, 0.0, 0.0, 1.0]), [0.9, 0.8, 0.7, 1.0]);
        assert_eq!(resolved, ASSET_PREVIEW_EDITOR_CLEAR_COLOR);
    }

    #[test]
    fn normal_viewport_keeps_scene_clear_color() {
        let scene = [0.1, 0.2, 0.3, 1.0];
        assert_eq!(
            resolve_viewport_clear_color(false, Some(scene), [0.9, 0.8, 0.7, 1.0]),
            scene
        );
    }

    #[test]
    fn runtime_debug_overlay_is_disabled_by_default() {
        assert!(!parse_runtime_debug_overlay_setting(None));
        assert!(!parse_runtime_debug_overlay_setting(Some("")));
    }

    #[test]
    fn runtime_debug_overlay_can_be_disabled_explicitly() {
        assert!(!parse_runtime_debug_overlay_setting(Some("0")));
        assert!(!parse_runtime_debug_overlay_setting(Some("false")));
        assert!(!parse_runtime_debug_overlay_setting(Some("off")));
    }
}
