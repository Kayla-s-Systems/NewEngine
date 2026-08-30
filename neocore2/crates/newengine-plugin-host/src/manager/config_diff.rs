#![forbid(unsafe_op_in_unsafe_fn)]

pub(crate) fn json_diff_keys_shallow_or_paths(
    content_type: &str,
    defaults_bytes: &[u8],
    effective_bytes: &[u8],
) -> Vec<String> {
    if defaults_bytes == effective_bytes || content_type != "application/json" {
        return Vec::new();
    }

    let defaults: serde_json::Value = match serde_json::from_slice(defaults_bytes) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let effective: serde_json::Value = match serde_json::from_slice(effective_bytes) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut out: Vec<String> = Vec::new();
    diff_json_paths(&defaults, &effective, "", 0, 4, 64, &mut out);
    out.sort();
    out.dedup();
    out
}

fn diff_json_paths(
    a: &serde_json::Value,
    b: &serde_json::Value,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    max_items: usize,
    out: &mut Vec<String>,
) {
    if out.len() >= max_items {
        return;
    }

    if depth >= max_depth {
        if a != b {
            out.push(prefix.to_owned());
        }
        return;
    }

    match (a, b) {
        (serde_json::Value::Object(ao), serde_json::Value::Object(bo)) => {
            for k in ao.keys() {
                if out.len() >= max_items {
                    return;
                }
                if !bo.contains_key(k) {
                    out.push(join_path(prefix, k));
                }
            }
            for (k, bv) in bo.iter() {
                if out.len() >= max_items {
                    return;
                }
                match ao.get(k) {
                    None => out.push(join_path(prefix, k)),
                    Some(av) => {
                        let p = join_path(prefix, k);
                        diff_json_paths(av, bv, &p, depth + 1, max_depth, max_items, out);
                    }
                }
            }
        }
        (serde_json::Value::Array(aa), serde_json::Value::Array(ba)) => {
            if aa.len() != ba.len() {
                out.push(prefix.to_owned());
                return;
            }
            for (i, (av, bv)) in aa.iter().zip(ba.iter()).enumerate() {
                if out.len() >= max_items {
                    return;
                }
                let p = if prefix.is_empty() {
                    format!("[{i}]")
                } else {
                    format!("{prefix}[{i}]")
                };
                diff_json_paths(av, bv, &p, depth + 1, max_depth, max_items, out);
            }
        }
        _ => {
            if a != b {
                out.push(prefix.to_owned());
            }
        }
    }
}

#[inline]
fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_json_uses_empty_fast_path() {
        let bytes = br#"{"nested":{"value":1}}"#;
        assert!(json_diff_keys_shallow_or_paths("application/json", bytes, bytes).is_empty());
    }

    #[test]
    fn nested_changes_are_reported_once_and_sorted() {
        let changed = json_diff_keys_shallow_or_paths(
            "application/json",
            br#"{"z":1,"nested":{"a":1,"b":2}}"#,
            br#"{"z":2,"nested":{"a":3,"c":4}}"#,
        );
        assert_eq!(changed, vec!["nested.a", "nested.b", "nested.c", "z"]);
    }
}
