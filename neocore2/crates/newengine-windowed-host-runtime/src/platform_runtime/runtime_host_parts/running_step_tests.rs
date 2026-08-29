#[cfg(test)]
mod presentation_loading_lifecycle_tests {
    use super::super::running_ui::effective_scene_launch_active;
    use newengine_core::render::SceneLaunchStatus;

    #[test]
    fn authored_frontend_suppresses_stale_scene_loading_status() {
        let status = SceneLaunchStatus::loading("Loading", "Preparing", "stale", 0.95);
        assert!(!effective_scene_launch_active(Some(&status), true));
    }

    #[test]
    fn scene_launch_status_remains_semantically_active_without_retained_preloader() {
        let status = SceneLaunchStatus::loading("Loading", "Preparing", "active", 0.95);
        assert!(effective_scene_launch_active(Some(&status), false));
    }

    #[test]
    fn inactive_scene_status_never_activates_scene_launch_gate() {
        let status = SceneLaunchStatus::inactive();
        assert!(!effective_scene_launch_active(Some(&status), false));
        assert!(!effective_scene_launch_active(None, false));
    }
}

#[cfg(test)]
mod animation_refresh_tests {
    use super::super::running_frontend_feedback::{
        animate_frontend_keycap_feedback, begin_frontend_keycap_feedback, frontend_action_keycap,
        frontend_keycap_press_amount, ui_dispatch_requests_exit, FrontendKeycapKind,
    };
    use super::super::running_settings::apply_frontend_setting_value;
    use super::super::running_ui::provider_draw_has_active_animation;
    use newengine_ui_api::{
        UiDrawList, UiEventDispatchFrame, UiPaintCommand, UiPaintNodeRef, UiRectPaintCommand,
    };
    use std::time::Duration;

    #[test]
    fn authored_exit_action_requests_native_close() {
        let mut frame = UiEventDispatchFrame::default();
        assert!(!ui_dispatch_requests_exit(&frame));
        frame.actions.push(newengine_ui_api::UiActionDispatch {
            action_id: "engine.lifecycle.exit".to_owned(),
            trigger: newengine_ui_api::UiNodeEventTrigger::Click,
            ..newengine_ui_api::UiActionDispatch::default()
        });
        assert!(ui_dispatch_requests_exit(&frame));
    }

    #[test]
    fn frontend_action_maps_to_expected_keycap_feedback() {
        assert_eq!(
            frontend_action_keycap("engine.settings.open"),
            (FrontendKeycapKind::Select, "OPEN")
        );
        assert_eq!(
            frontend_action_keycap("settings.apply"),
            (FrontendKeycapKind::Select, "APPLY")
        );
        assert_eq!(
            frontend_action_keycap("ui.back"),
            (FrontendKeycapKind::Back, "RETURN")
        );
        assert_eq!(
            frontend_action_keycap("engine.lifecycle.exit"),
            (FrontendKeycapKind::Back, "EXITING")
        );
    }

    #[test]
    fn keycap_feedback_changes_only_matching_keycap_commands() {
        let mut draw = newengine_ui_api::UiDrawList::new();
        draw.paint.push(newengine_ui_api::UiPaintCommand::Image(
            newengine_ui_api::UiImagePaintCommand {
                node: newengine_ui_api::UiPaintNodeRef {
                    node_id: "main.hint.select.keycap".to_owned(),
                    ..Default::default()
                },
                rect: [100.0, 100.0, 42.0, 33.0],
                tint_rgba: 0xffff_ffff,
                ..Default::default()
            },
        ));
        draw.paint.push(newengine_ui_api::UiPaintCommand::Image(
            newengine_ui_api::UiImagePaintCommand {
                node: newengine_ui_api::UiPaintNodeRef {
                    node_id: "main.hint.back.keycap".to_owned(),
                    ..Default::default()
                },
                rect: [200.0, 100.0, 69.0, 33.0],
                tint_rgba: 0xffff_ffff,
                ..Default::default()
            },
        ));
        begin_frontend_keycap_feedback(FrontendKeycapKind::Select, "OPEN");
        std::thread::sleep(Duration::from_millis(40));
        animate_frontend_keycap_feedback(&mut draw);
        let select = draw
            .paint
            .commands
            .iter()
            .find_map(|command| match command {
                newengine_ui_api::UiPaintCommand::Image(image)
                    if image.node.node_id == "main.hint.select.keycap" =>
                {
                    Some(image)
                }
                _ => None,
            })
            .expect("transformed select keycap");
        let back = draw
            .paint
            .commands
            .iter()
            .find_map(|command| match command {
                newengine_ui_api::UiPaintCommand::Image(image)
                    if image.node.node_id == "main.hint.back.keycap" =>
                {
                    Some(image)
                }
                _ => None,
            })
            .expect("untouched back keycap");
        assert!(draw.paint.commands.iter().any(|command| matches!(
            command,
            newengine_ui_api::UiPaintCommand::Rect(rect)
                if rect.node.role == "keycap-pressed-well"
        )));
        assert!(
            select.rect[1] > 106.0,
            "keycap must visibly travel downward"
        );
        assert!(select.rect[2] < 39.5, "keycap must compress horizontally");
        assert!(select.rect[3] < 28.5, "keycap must compress vertically");
        assert_eq!(back.rect, [200.0, 100.0, 69.0, 33.0]);
    }

    #[test]
    fn keycap_press_curve_has_attack_hold_and_release() {
        assert_eq!(frontend_keycap_press_amount(Duration::ZERO), 0.0);
        assert!(frontend_keycap_press_amount(Duration::from_millis(25)) > 0.4);
        assert_eq!(frontend_keycap_press_amount(Duration::from_millis(80)), 1.0);
        assert_eq!(
            frontend_keycap_press_amount(Duration::from_millis(180)),
            1.0
        );
        let release = frontend_keycap_press_amount(Duration::from_millis(300));
        assert!(release > 0.0 && release < 1.0);
    }

    #[test]
    fn frontend_setting_values_patch_existing_config_fields() {
        let mut document = serde_json::json!({
            "startup_settings": {
                "display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed"},
                "graphics": {"preset": "cinematic", "bloom_enabled": true}
            },
            "window": {"display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed"}},
            "plugins": {
                "newengine": {"startup_window": {"display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed", "fullscreen": false}}},
                "engine.platform.winit": {"display": {"vsync": false, "render_scale": 1.0, "window_mode": "windowed", "fullscreen": false}}
            }
        });
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.display.fullscreen",
            &serde_json::json!(true),
        ));
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.display.vsync",
            &serde_json::json!(true),
        ));
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.display.render_scale",
            &serde_json::json!(1.25),
        ));
        assert!(apply_frontend_setting_value(
            &mut document,
            "settings.graphics.bloom",
            &serde_json::json!(false),
        ));
        assert_eq!(
            document.pointer("/startup_settings/display/window_mode"),
            Some(&serde_json::json!("exclusive_fullscreen"))
        );
        assert_eq!(
            document.pointer("/startup_settings/display/vsync"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            document.pointer("/startup_settings/display/render_scale"),
            Some(&serde_json::json!(1.25))
        );
        assert_eq!(
            document.pointer("/startup_settings/graphics/bloom_enabled"),
            Some(&serde_json::json!(false))
        );
        assert_eq!(
            document.pointer("/startup_settings/graphics/preset"),
            Some(&serde_json::json!("custom"))
        );
    }

    #[test]
    fn provider_draw_detects_animated_hover_underline() {
        let mut draw = UiDrawList::new();
        assert!(!provider_draw_has_active_animation(&draw));
        draw.paint.push(UiPaintCommand::Rect(UiRectPaintCommand {
            node: UiPaintNodeRef {
                role: "hover-underline-animated".to_owned(),
                ..UiPaintNodeRef::default()
            },
            rect: [0.0, 0.0, 20.0, 2.0],
            color: 0xffff_ffff,
            clip_rect: None,
        }));
        assert!(provider_draw_has_active_animation(&draw));
    }

    #[test]
    fn selected_underline_does_not_force_continuous_refresh() {
        let mut draw = UiDrawList::new();
        draw.paint.push(UiPaintCommand::Rect(UiRectPaintCommand {
            node: UiPaintNodeRef {
                role: "selected-underline".to_owned(),
                ..UiPaintNodeRef::default()
            },
            rect: [0.0, 0.0, 20.0, 2.0],
            color: 0xffff_ffff,
            clip_rect: None,
        }));
        assert!(!provider_draw_has_active_animation(&draw));
    }
}
