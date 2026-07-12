use super::*;

fn attr_f32(open: &str, keys: &[&str]) -> Option<f32> {
    keys.iter()
        .find_map(|key| attr_value(open, key))
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
}

fn attr_i32(open: &str, keys: &[&str]) -> Option<i32> {
    keys.iter()
        .find_map(|key| attr_value(open, key))
        .and_then(|value| value.trim().parse::<i32>().ok())
}

fn attr_bool_value(open: &str, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        let value = attr_value(open, key)?;
        match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn put_f32_prop(node: &mut UiNodeRequest, key: &str, value: f32) {
    if let Some(number) = serde_json::Number::from_f64(value as f64) {
        node.props
            .insert(key.to_owned(), serde_json::Value::Number(number));
    }
}

fn put_bool_prop(node: &mut UiNodeRequest, key: &str, value: bool) {
    node.props
        .insert(key.to_owned(), serde_json::Value::Bool(value));
}

pub(super) fn apply_layout_attrs(node: &mut UiNodeRequest, open: &str) {
    if let Some(value) = attr_f32(open, &["x_px", "x"]) {
        node.layout.x_px = Some(value);
        put_f32_prop(node, "x_px", value);
    }
    if let Some(value) = attr_f32(open, &["y_px", "y"]) {
        node.layout.y_px = Some(value);
        put_f32_prop(node, "y_px", value);
    }
    if let Some(value) = attr_f32(open, &["w_px", "width_px", "width"]) {
        node.layout.w_px = Some(value);
        put_f32_prop(node, "w_px", value);
    }
    if let Some(value) = attr_f32(open, &["h_px", "height_px", "height"]) {
        node.layout.h_px = Some(value);
        put_f32_prop(node, "h_px", value);
    }

    if let Some(value) = attr_f32(open, &["min_w_px", "min_width_px"]) {
        node.layout.min_size_px[0] = value;
        put_f32_prop(node, "min_w_px", value);
    }
    if let Some(value) = attr_f32(open, &["min_h_px", "min_height_px"]) {
        node.layout.min_size_px[1] = value;
        put_f32_prop(node, "min_h_px", value);
    }
    if let Some(value) = attr_f32(open, &["max_w_px", "max_width_px"]) {
        node.layout.max_size_px[0] = value;
        put_f32_prop(node, "max_w_px", value);
    }
    if let Some(value) = attr_f32(open, &["max_h_px", "max_height_px"]) {
        node.layout.max_size_px[1] = value;
        put_f32_prop(node, "max_h_px", value);
    }
    if let Some(value) = attr_f32(open, &["grow", "flex_grow"]) {
        node.layout.grow = value.max(0.0);
        put_f32_prop(node, "grow", node.layout.grow);
    }
    if let Some(value) = attr_f32(open, &["shrink", "flex_shrink"]) {
        node.layout.shrink = value.max(0.0);
        put_f32_prop(node, "shrink", node.layout.shrink);
    }
    if let Some(value) = attr_i32(open, &["order"]) {
        node.layout.order = value;
        node.props
            .insert("order".to_owned(), serde_json::json!(value));
    }
    if let Some(value) = attr_value(open, "slot").filter(|it| !it.trim().is_empty()) {
        node.layout.slot = value;
        node.props.insert(
            "slot".to_owned(),
            serde_json::Value::String(node.layout.slot.clone()),
        );
    }
    if let Some(value) = attr_bool_value(open, &["resizable"]) {
        node.layout.resizable = value;
        put_bool_prop(node, "resizable", value);
    }
    if let Some(value) = attr_bool_value(open, &["detachable"]) {
        node.layout.detachable = value;
        put_bool_prop(node, "detachable", value);
    }
}
