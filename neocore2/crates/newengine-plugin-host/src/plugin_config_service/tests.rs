use super::*;
    use serde_json::json;

    fn make_store(overrides: HashMap<String, Value>) -> PluginConfigStore {
        PluginConfigStore::new(overrides)
    }

    #[test]
    fn log_summary_truncates_string_values_safely() {
        let long_value = "x".repeat(LOG_VALUE_MAX_BYTES + 32);
        let summarized = summarize_value_for_log(&json!(long_value));

        assert!(summarized.ends_with(LOG_TRUNCATION_SUFFIX));
        assert_eq!(summarized.len(), LOG_VALUE_MAX_BYTES);
    }

    #[test]
    fn log_summary_truncates_multibyte_values_on_char_boundary() {
        let long_value = "Ж".repeat(LOG_VALUE_MAX_BYTES);
        let summarized = summarize_value_for_log(&json!(long_value));

        assert!(summarized.ends_with(LOG_TRUNCATION_SUFFIX));
        assert!(summarized.len() <= LOG_VALUE_MAX_BYTES);
        assert!(summarized.is_char_boundary(summarized.len()));
    }

    #[test]
    fn resolves_exact_flat_plugin_override() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "engine.logging.chronicle".to_owned(),
            json!({
                "timestamp": "millis",
                "format": { "preset": "aaa" }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("engine.logging.chronicle");

        assert_eq!(got["timestamp"], json!("millis"));
        assert_eq!(got["format"]["preset"], json!("aaa"));
    }

    #[test]
    fn resolves_nested_domain_wrapped_plugin_override() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "newengine".to_owned(),
            json!({
                "platform": {
                    "winit": {
                        "title": "NewEngine Editor",
                        "placement": {
                            "mode": "centered",
                            "x": 0,
                            "y": -24
                        }
                    }
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("engine.platform.winit");

        assert_eq!(got["title"], json!("NewEngine Editor"));
        assert_eq!(got["placement"]["mode"], json!("centered"));
        assert_eq!(got["placement"]["y"], json!(-24));
    }

    #[test]
    fn exact_flat_override_wins_and_nested_fills_missing_keys() {
        let mut overrides = HashMap::default();

        overrides.insert(
            "newengine".to_owned(),
            json!({
                "logging": {
                    "timestamp": "millis",
                    "sources": {
                        "console": {
                            "enabled": true,
                            "level": "info"
                        },
                        "file": {
                            "enabled": true,
                            "path": "logs/default.log"
                        }
                    }
                }
            }),
        );

        overrides.insert(
            "engine.logging.chronicle".to_owned(),
            json!({
                "sources": {
                    "file": {
                        "path": "logs/log.log",
                        "mode": "truncate"
                    }
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("engine.logging.chronicle");

        assert_eq!(got["timestamp"], json!("millis"));
        assert_eq!(got["sources"]["console"]["enabled"], json!(true));
        assert_eq!(got["sources"]["console"]["level"], json!("info"));
        assert_eq!(got["sources"]["file"]["path"], json!("logs/log.log"));
        assert_eq!(got["sources"]["file"]["mode"], json!("truncate"));
        assert_eq!(got["sources"]["file"]["enabled"], json!(true));
    }

    #[test]
    fn resolves_nested_domain_wrapped_dotted_leaf_key() {
        let mut overrides = HashMap::default();

        overrides.insert(
            "newengine".to_owned(),
            json!({
                "platform.winit": {
                    "title": "Editor",
                    "width": 1600,
                    "height": 900,
                    "placement": {
                        "mode": "centered",
                        "x": 0,
                        "y": -24
                    },
                    "icon": "ui/engine.ico"
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("engine.platform.winit");

        assert_eq!(got["title"], json!("Editor"));
        assert_eq!(got["width"], json!(1600));
        assert_eq!(got["height"], json!(900));
        assert_eq!(got["placement"]["mode"], json!("centered"));
        assert_eq!(got["placement"]["x"], json!(0));
        assert_eq!(got["placement"]["y"], json!(-24));
        assert_eq!(got["icon"], json!("ui/engine.ico"));
    }

    #[test]
    fn resolves_grouped_exact_prefix_override_for_renderer_backend() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "engine.render".to_owned(),
            json!({
                "vulkan": {
                    "clear_color": [0.02, 0.025, 0.035, 1.0],
                    "debug_text": "NewEngine | Vulkan"
                }
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("engine.render.vulkan");

        assert_eq!(got["clear_color"], json!([0.02, 0.025, 0.035, 1.0]));
        assert_eq!(got["debug_text"], json!("NewEngine | Vulkan"));
    }

    #[test]
    fn exact_renderer_backend_override_wins_over_group_prefix() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "engine.render".to_owned(),
            json!({
                "vulkan": {
                    "clear_color": [0.02, 0.025, 0.035, 1.0],
                    "debug_text": "Grouped"
                }
            }),
        );
        overrides.insert(
            "engine.render.vulkan".to_owned(),
            json!({
                "debug_text": "Exact"
            }),
        );

        let store = make_store(overrides);
        let got = store.resolve_plugin_overrides("engine.render.vulkan");

        assert_eq!(got["clear_color"], json!([0.02, 0.025, 0.035, 1.0]));
        assert_eq!(got["debug_text"], json!("Exact"));
    }

    #[test]
    fn plugin_config_store_is_isolated_per_host_context() {
        let a = crate::host_context::create_host_context();
        let b = crate::host_context::create_host_context();

        let mut overrides_a = HashMap::default();
        overrides_a.insert("engine.render.vulkan".to_owned(), json!({"marker": "A"}));
        let mut overrides_b = HashMap::default();
        overrides_b.insert("engine.render.vulkan".to_owned(), json!({"marker": "B"}));

        crate::host_context::with_host_context(&a, || init_plugin_config_service(overrides_a));
        crate::host_context::with_host_context(&b, || init_plugin_config_service(overrides_b));

        crate::host_context::with_host_context(&a, || {
            assert_eq!(
                get_plugin_overrides_with_env("engine.render.vulkan")["marker"],
                json!("A")
            );
        });
        crate::host_context::with_host_context(&b, || {
            assert_eq!(
                get_plugin_overrides_with_env("engine.render.vulkan")["marker"],
                json!("B")
            );
        });
    }

    #[test]
    fn resolved_overrides_are_cached_by_plugin_id() {
        let mut overrides = HashMap::default();
        overrides.insert(
            "engine.render.vulkan".to_owned(),
            json!({"debug_text": "Cached"}),
        );
        let store = make_store(overrides);

        assert!(store.resolved_cache.lock().expect("cache").is_empty());
        assert_eq!(
            store.resolve_plugin_overrides("engine.render.vulkan")["debug_text"],
            json!("Cached")
        );
        assert_eq!(store.resolved_cache.lock().expect("cache").len(), 1);
        assert_eq!(
            store.resolve_plugin_overrides("engine.render.vulkan")["debug_text"],
            json!("Cached")
        );
        assert_eq!(store.resolved_cache.lock().expect("cache").len(), 1);
    }

    #[test]
    fn returns_empty_object_when_override_is_missing() {
        let store = make_store(HashMap::default());
        let got = store.resolve_plugin_overrides("newengine.missing");

        assert_eq!(got, json!({}));
    }
