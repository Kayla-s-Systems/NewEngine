use super::*;

fn weapon_namespace(entry: &serde_json::Value) -> Option<&serde_json::Value> {
    entry
        .get("arbitrary_metadata")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("newengine.weapon"))
        .or_else(|| {
            entry
                .get("metadata")
                .and_then(|value| value.get("newengine.weapon"))
        })
}

fn get<'a>(value: &'a serde_json::Value, object: &str, key: &str) -> Option<&'a serde_json::Value> {
    value.get(object)?.get(key)
}

fn string(value: &serde_json::Value, object: &str, key: &str) -> Option<String> {
    get(value, object, key)?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn string_list(value: &serde_json::Value, object: &str, key: &str) -> Option<Vec<String>> {
    let value = get(value, object, key)?;
    let values = if let Some(values) = value.as_array() {
        values
            .iter()
            .filter_map(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    } else {
        value
            .as_str()?
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    };
    (!values.is_empty()).then_some(values)
}

fn string_map(
    value: &serde_json::Value,
    object: &str,
    key: &str,
) -> Option<std::collections::BTreeMap<String, String>> {
    let value = get(value, object, key)?;
    let mut out = std::collections::BTreeMap::new();
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            let Some(value) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            if !key.is_empty() {
                out.insert(key, value.to_owned());
            }
        }
    } else if let Some(raw) = value.as_str() {
        for entry in raw
            .split(';')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let Some((key, mapped)) = entry.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let mapped = mapped.trim();
            if !key.is_empty() && !mapped.is_empty() {
                out.insert(key, mapped.to_owned());
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

fn f32_value(value: &serde_json::Value, object: &str, key: &str) -> Option<f32> {
    let value = get(value, object, key)?;
    value
        .as_f64()
        .map(|value| value as f32)
        .or_else(|| value.as_str()?.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
}

fn u32_value(value: &serde_json::Value, object: &str, key: &str) -> Option<u32> {
    let value = get(value, object, key)?;
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse::<u32>().ok())
}

fn vec3(value: &serde_json::Value, object: &str, key: &str) -> Option<[f32; 3]> {
    let value = get(value, object, key)?;
    if let Some(values) = value.as_array() {
        if values.len() != 3 {
            return None;
        }
        let mut out = [0.0; 3];
        for (index, value) in values.iter().enumerate() {
            out[index] = value.as_f64()? as f32;
            if !out[index].is_finite() {
                return None;
            }
        }
        return Some(out);
    }
    let raw = value.as_str()?;
    let values = raw
        .split(',')
        .map(str::trim)
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3 && values.iter().all(|value| value.is_finite()))
        .then(|| [values[0], values[1], values[2]])
}

fn vec4(value: &serde_json::Value, object: &str, key: &str) -> Option<[f32; 4]> {
    let value = get(value, object, key)?;
    let values = if let Some(values) = value.as_array() {
        values
            .iter()
            .map(|value| value.as_f64().map(|v| v as f32))
            .collect::<Option<Vec<_>>>()?
    } else {
        value
            .as_str()?
            .split(',')
            .map(str::trim)
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .ok()?
    };
    (values.len() == 4 && values.iter().all(|value| value.is_finite()))
        .then(|| [values[0], values[1], values[2], values[3]])
}

fn bool_value(value: &serde_json::Value, object: &str, key: &str) -> Option<bool> {
    let value = get(value, object, key)?;
    value.as_bool().or_else(|| {
        let raw = value.as_str()?.trim().to_ascii_lowercase();
        match raw.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

fn parse_component_graph(
    namespace: &serde_json::Value,
) -> Result<Option<AuthoredWeaponComponentGraphDefinition>, String> {
    let points_raw = string(namespace, "component_points", "points");
    let components_raw = string(namespace, "components", "definitions");
    let installed = string_map(namespace, "components", "installed").unwrap_or_default();
    if points_raw.is_none() && components_raw.is_none() && installed.is_empty() {
        return Ok(None);
    }
    let mut graph = AuthoredWeaponComponentGraphDefinition::default();
    if let Some(raw) = points_raw {
        for record in raw
            .split(';')
            .map(str::trim)
            .filter(|record| !record.is_empty())
        {
            let fields = record.split('|').map(str::trim).collect::<Vec<_>>();
            if fields.len() < 2 {
                return Err(format!("invalid weapon component point record '{record}'"));
            }
            graph.points.push(AuthoredWeaponComponentPointDefinition {
                id: fields[0].to_owned(),
                attach_joint: fields[1].to_owned(),
                allowed_components: fields
                    .get(2)
                    .copied()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect(),
            });
        }
    }
    if let Some(raw) = components_raw {
        for record in raw
            .split(';')
            .map(str::trim)
            .filter(|record| !record.is_empty())
        {
            let fields = record.split('|').map(str::trim).collect::<Vec<_>>();
            if fields.len() < 2 {
                return Err(format!("invalid weapon component record '{record}'"));
            }
            let scalar = |index: usize| -> Result<f32, String> {
                let raw = fields.get(index).copied().unwrap_or("");
                if raw.is_empty() {
                    return Ok(1.0);
                }
                raw.parse::<f32>()
                    .map_err(|_| format!("invalid component modifier '{}' in '{record}'", raw))
            };
            graph.components.push(AuthoredWeaponComponentDefinition {
                id: fields[0].to_owned(),
                slot: fields[1].to_owned(),
                model_ref: fields.get(2).copied().unwrap_or("").to_owned(),
                audio_override: fields.get(3).copied().unwrap_or("").to_owned(),
                muzzle_vfx_override: fields.get(4).copied().unwrap_or("").to_owned(),
                tracer_vfx_override: fields.get(5).copied().unwrap_or("").to_owned(),
                stat_modifiers: Vec::new(),
                modifiers: AuthoredWeaponComponentModifiers {
                    accuracy_multiplier: scalar(6)?,
                    recoil_multiplier: scalar(7)?,
                    damage_multiplier: scalar(8)?,
                    falloff_multiplier: scalar(9)?,
                    muzzle_velocity_multiplier: scalar(10)?,
                    penetration_multiplier: scalar(11)?,
                    audio_gain_multiplier: scalar(12)?,
                    presentation_offset_local: [
                        fields
                            .get(13)
                            .copied()
                            .unwrap_or("0")
                            .parse::<f32>()
                            .unwrap_or(0.0),
                        fields
                            .get(14)
                            .copied()
                            .unwrap_or("0")
                            .parse::<f32>()
                            .unwrap_or(0.0),
                        fields
                            .get(15)
                            .copied()
                            .unwrap_or("0")
                            .parse::<f32>()
                            .unwrap_or(0.0),
                    ],
                },
            });
        }
    }
    graph.default_installed = installed;
    // Validate through the runtime graph compiler now so malformed YTYP never reaches gameplay.
    let _ = graph.compile()?;
    Ok(Some(graph))
}
