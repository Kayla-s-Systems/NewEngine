fn collect_plugin_override_report_entries(
    root_key: &str,
    value: &serde_json::Value,
    out: &mut Vec<StartupPluginOverride>,
) {
    fn visit(
        path: String,
        value: &serde_json::Value,
        out: &mut Vec<StartupPluginOverride>,
        is_root: bool,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                let is_leaf_like = map.is_empty()
                    || map.keys().any(|key| key.contains('.'))
                    || map.values().any(|v| !v.is_object());

                if is_leaf_like {
                    out.push(StartupPluginOverride {
                        plugin_id: path,
                        key: "plugins.*",
                        from: "<plugin defaults>".to_owned(),
                        to: summarize_json(value),
                    });
                    return;
                }

                if is_root && map.is_empty() {
                    out.push(StartupPluginOverride {
                        plugin_id: path,
                        key: "plugins.*",
                        from: "<plugin defaults>".to_owned(),
                        to: summarize_json(value),
                    });
                    return;
                }

                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                for key in keys {
                    if let Some(child) = map.get(key) {
                        visit(format!("{path}.{key}"), child, out, false);
                    }
                }
            }
            _ => {
                out.push(StartupPluginOverride {
                    plugin_id: path,
                    key: "plugins.*",
                    from: "<plugin defaults>".to_owned(),
                    to: summarize_json(value),
                });
            }
        }
    }

    visit(root_key.to_owned(), value, out, true);
}

fn dedup_plugin_override_report_entries(entries: &mut Vec<StartupPluginOverride>) {
    let mut by_id =
        newengine_math::collections_prelude::NeBTreeMap::<String, StartupPluginOverride>::new();
    for entry in entries.drain(..) {
        by_id.insert(entry.plugin_id.clone(), entry);
    }
    *entries = by_id.into_values().collect();
}
