use super::*;

pub(super) fn read_config(path: &Path) -> (Value, Option<String>) {
    match fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => match serde_json::from_str::<Value>(&text) {
            Ok(value) => (value, None),
            Err(err) => (
                Value::Object(Map::new()),
                Some(format!("config.json parse failed: {err}. The editor opened with an empty config; press Cancel to avoid overwriting.")),
            ),
        },
        Ok(_) => (Value::Object(Map::new()), None),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (Value::Object(Map::new()), None),
        Err(err) => (
            Value::Object(Map::new()),
            Some(format!("config.json read failed: {err}. The editor opened with defaults.")),
        ),
    }
}

pub(super) fn display_window_mode_from_config(config: &Value) -> String {
    let raw_mode = first_string_segments(config, &[
        &["plugins", "newengine", "platform.winit", "display", "window_mode"],
        &["plugins", "newengine", "startup_window", "display", "window_mode"],
        &["window", "display", "window_mode"],
    ], "windowed");
    let fullscreen = first_bool_segments(config, &[
        &["plugins", "newengine", "platform.winit", "display", "fullscreen"],
        &["plugins", "newengine", "startup_window", "display", "fullscreen"],
        &["window", "display", "fullscreen"],
    ], false);
    let borderless = first_bool_segments(config, &[
        &["plugins", "newengine", "platform.winit", "display", "borderless_fullscreen"],
        &["plugins", "newengine", "startup_window", "display", "borderless_fullscreen"],
        &["window", "display", "borderless_fullscreen"],
    ], false);

    let mut mode = normalize_window_mode(raw_mode);
    if borderless {
        mode = "borderless".to_owned();
    } else if fullscreen && mode == "windowed" {
        mode = "exclusive_fullscreen".to_owned();
    }
    mode
}

pub(super) fn normalize_window_mode(value: impl AsRef<str>) -> String {
    match value.as_ref().trim().to_ascii_lowercase().as_str() {
        "borderless" | "borderless_fullscreen" => "borderless".to_owned(),
        "exclusive" | "exclusive_fullscreen" | "fullscreen" => "exclusive_fullscreen".to_owned(),
        _ => "windowed".to_owned(),
    }
}

pub(super) fn plugin_enabled_from_config(config: &Value, plugin_id: &str, fallback: bool) -> bool {
    get_plugin_field(config, plugin_id, "host.enabled")
        .and_then(Value::as_bool)
        .or_else(|| get_plugin_field(config, plugin_id, "enabled").and_then(Value::as_bool))
        .unwrap_or(fallback)
}

pub(super) fn collect_plugin_tabs(config: &Value) -> Vec<PluginTab> {
    let Some(array) = get_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"]).and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut tabs = Vec::new();
    for item in array {
        let plugin_id = value_string(item.get("plugin_id"), "unknown.plugin");
        let title = value_string(item.get("title"), &plugin_id);
        let category = value_string(item.get("category"), "Plugin");
        let source = value_string(item.get("source"), "config.json");
        let tab_enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        let enabled = plugin_enabled_from_config(config, &plugin_id, tab_enabled);
        let mut fields = Vec::new();
        if let Some(schema_fields) = item
            .get("schema")
            .and_then(|schema| schema.get("fields"))
            .and_then(Value::as_array)
        {
            for raw_field in schema_fields {
                let path = value_string(raw_field.get("path"), "");
                if path.is_empty() {
                    continue;
                }
                let key = value_string(raw_field.get("key"), &path);
                let label_text = value_string(raw_field.get("label"), &key);
                let kind = value_string(raw_field.get("kind"), "string");
                let default_label = raw_field
                    .get("default_label")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                let mut options = Vec::new();
                if let Some(raw_options) = raw_field.get("options").and_then(Value::as_array) {
                    for option in raw_options {
                        let value = value_string(option.get("value"), "");
                        if value.is_empty() {
                            continue;
                        }
                        let label = value_string(option.get("label"), &value);
                        options.push(SelectOption { value, label });
                    }
                }
                fields.push(SchemaField {
                    key,
                    path,
                    label: label_text,
                    kind,
                    options,
                    default_label,
                });
            }
        }
        tabs.push(PluginTab { plugin_id, title, category, source, enabled, fields });
    }
    tabs.sort_by(|a, b| a.title.cmp(&b.title));
    tabs
}

pub(super) fn plugin_field_current(config: &Value, plugin_id: &str, field: &SchemaField) -> Option<Value> {
    if let Some(current) = plugin_schema_field(config, plugin_id, &field.path, "current") {
        return Some(current.clone());
    }
    if let Some(default) = plugin_schema_field(config, plugin_id, &field.path, "default") {
        return Some(default.clone());
    }
    get_plugin_field(config, plugin_id, &field.path).cloned()
}

pub(super) fn plugin_schema_field<'a>(config: &'a Value, plugin_id: &str, field_path: &str, key: &str) -> Option<&'a Value> {
    let tabs = get_segments(config, &["plugins", "newengine", "startup_window", "plugin_tabs"])?.as_array()?;
    for tab in tabs {
        if tab.get("plugin_id").and_then(Value::as_str) != Some(plugin_id) {
            continue;
        }
        let fields = tab.get("schema")?.get("fields")?.as_array()?;
        for field in fields {
            if field.get("path").and_then(Value::as_str) == Some(field_path) {
                return field.get(key);
            }
        }
    }
    None
}

pub(super) fn plugin_field_key(plugin_id: &str, field_path: &str) -> String {
    format!("plugin::{plugin_id}::{field_path}")
}

pub(super) fn get_plugin_field<'a>(config: &'a Value, plugin_id: &str, rel_path: &str) -> Option<&'a Value> {
    let plugins = config.get("plugins")?.as_object()?;
    let mut id_parts = plugin_id.split('.');
    let namespace = id_parts.next()?;
    let tail_parts: Vec<&str> = id_parts.collect();
    let namespace_value = plugins.get(namespace)?;
    if tail_parts.is_empty() {
        return get_path(namespace_value, rel_path);
    }
    let tail_literal = tail_parts.join(".");
    if let Some(root) = namespace_value.get(&tail_literal) {
        return get_path(root, rel_path);
    }
    let mut node = namespace_value;
    for part in tail_parts {
        node = node.get(part)?;
    }
    get_path(node, rel_path)
}

pub(super) fn set_plugin_field(config: &mut Value, plugin_id: &str, rel_path: &str, value: Value) {
    ensure_object(config);
    let mut id_parts = plugin_id.split('.');
    let Some(namespace) = id_parts.next() else { return; };
    let tail_parts: Vec<&str> = id_parts.collect();
    let plugins = ensure_child_object(config, "plugins");
    let namespace_value = ensure_child_object(plugins, namespace);
    if tail_parts.is_empty() {
        set_path(namespace_value, rel_path, value);
        return;
    }
    let tail_literal = tail_parts.join(".");
    let use_literal = namespace_value
        .as_object()
        .and_then(|object| object.get(&tail_literal))
        .is_some()
        || tail_literal.starts_with("platform.");
    if use_literal {
        let root = ensure_child_object(namespace_value, &tail_literal);
        set_path(root, rel_path, value);
        return;
    }
    let mut node = namespace_value;
    for part in tail_parts {
        node = ensure_child_object(node, part);
    }
    set_path(node, rel_path, value);
}

pub(super) fn value_string(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(other) => serde_json::to_string(other).unwrap_or_else(|_| default.to_owned()),
        None => default.to_owned(),
    }
}

pub(super) fn current_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => String::new(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

pub(super) fn value_string_segments(config: &Value, segments: &[&str], default: &str) -> String {
    value_string(get_segments(config, segments), default)
}

pub(super) fn value_bool_segments(config: &Value, segments: &[&str], default: bool) -> bool {
    get_segments(config, segments).and_then(Value::as_bool).unwrap_or(default)
}

pub(super) fn value_i64_segments(config: &Value, segments: &[&str], default: i64) -> i64 {
    get_segments(config, segments).and_then(Value::as_i64).unwrap_or(default)
}

pub(super) fn value_f64_segments(config: &Value, segments: &[&str], default: f64) -> f64 {
    get_segments(config, segments).and_then(Value::as_f64).unwrap_or(default)
}

pub(super) fn first_string_segments(config: &Value, paths: &[&[&str]], default: &str) -> String {
    for path in paths {
        if let Some(value) = get_segments(config, path) {
            return value_string(Some(value), default);
        }
    }
    default.to_owned()
}

pub(super) fn first_i64_segments(config: &Value, paths: &[&[&str]], default: i64) -> i64 {
    for path in paths {
        if let Some(value) = get_segments(config, path).and_then(Value::as_i64) {
            return value;
        }
    }
    default
}

pub(super) fn first_bool_segments(config: &Value, paths: &[&[&str]], default: bool) -> bool {
    for path in paths {
        if let Some(value) = get_segments(config, path).and_then(Value::as_bool) {
            return value;
        }
    }
    default
}

pub(super) fn first_f64_segments(config: &Value, paths: &[&[&str]], default: f64) -> f64 {
    for path in paths {
        if let Some(value) = get_segments(config, path).and_then(Value::as_f64) {
            return value;
        }
    }
    default
}

pub(super) fn get_segments<'a>(root: &'a Value, segments: &[&str]) -> Option<&'a Value> {
    let mut node = root;
    for segment in segments {
        node = node.get(*segment)?;
    }
    Some(node)
}

pub(super) fn set_segments(root: &mut Value, segments: &[&str], value: Value) {
    if segments.is_empty() {
        *root = value;
        return;
    }
    ensure_object(root);
    let mut node = root;
    for segment in &segments[..segments.len() - 1] {
        node = ensure_child_object(node, segment);
    }
    let leaf = segments[segments.len() - 1];
    ensure_object(node).insert(leaf.to_owned(), value);
}

pub(super) fn get_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut node = root;
    for part in path.split('.').filter(|part| !part.is_empty()) {
        node = node.get(part)?;
    }
    Some(node)
}

pub(super) fn set_path(root: &mut Value, path: &str, value: Value) {
    let parts: Vec<&str> = path.split('.').filter(|part| !part.is_empty()).collect();
    set_segments(root, &parts, value);
}

pub(super) fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().expect("value forced to object")
}

pub(super) fn ensure_child_object<'a>(value: &'a mut Value, key: &str) -> &'a mut Value {
    let object = ensure_object(value);
    let child = object.entry(key.to_owned()).or_insert_with(|| Value::Object(Map::new()));
    if !child.is_object() {
        *child = Value::Object(Map::new());
    }
    child
}

pub(super) fn parse_i64(text: &str, default: i64) -> i64 {
    text.trim().parse::<i64>().unwrap_or(default)
}

pub(super) fn parse_f64(text: &str, default: f64) -> f64 {
    text.trim().parse::<f64>().unwrap_or(default)
}

pub(super) fn parse_json_or_string(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(text.to_owned()))
}

pub(super) fn number_value(value: f64) -> Value {
    Number::from_f64(value).map(Value::Number).unwrap_or(Value::Null)
}

pub(super) fn graphics_profile_id(profile: &str) -> &'static str {
    match profile {
        "safe_mode" => "newengine.render.runtime.tier.safe",
        "legacy_gpu" => "newengine.render.runtime.tier.legacy",
        "modern_gpu" => "newengine.render.runtime.tier.gtx",
        "rtx" | "rtx_raytracing_capable" => "newengine.render.runtime.tier.rtx",
        "developer_diagnostics" => "newengine.render.runtime.tier.developer_diagnostics",
        _ => "newengine.render.runtime.tier.auto",
    }
}

pub(super) fn option_label(options: &[SelectOption], selected: &str) -> String {
    options
        .iter()
        .find(|option| option.value == selected)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| selected.to_owned())
}