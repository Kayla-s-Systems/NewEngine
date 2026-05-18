#![forbid(unsafe_op_in_unsafe_fn)]

pub(crate) fn parse_methods_from_description(description: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value = serde_json::from_str(description).map_err(|e| e.to_string())?;
    let Some(methods) = v.get("methods") else {
        return Err("missing methods".to_owned());
    };

    let mut out = Vec::new();
    if let Some(methods_array) = methods.as_array() {
        out.reserve(methods_array.len());
        for item in methods_array {
            if let Some(name) = item.as_str() {
                out.push(name.to_owned());
                continue;
            }
            if let Some(name) = item.get("name").and_then(|x| x.as_str()) {
                out.push(name.to_owned());
            }
        }
    } else if let Some(methods_object) = methods.as_object() {
        out.reserve(methods_object.len());
        for name in methods_object.keys() {
            out.push(name.to_owned());
        }
    } else {
        return Err("methods must be an array or object".to_owned());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

pub(crate) fn method_statuses(methods: &[&str]) -> Vec<String> {
    methods
        .iter()
        .map(|method| {
            let label = method
                .rsplit_once('.')
                .map(|(_, tail)| tail)
                .unwrap_or(method);
            format!("{label}=yes")
        })
        .collect()
}
