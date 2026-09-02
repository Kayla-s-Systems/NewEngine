fn audio_metadata_namespace(entry: &serde_json::Value) -> Option<&serde_json::Value> {
    metadata_namespace(entry, "newengine.audio")
}

fn string_or_array(value: &serde_json::Value) -> Vec<String> {
    if let Some(value) = value.as_str() {
        let value = value.trim().to_ascii_lowercase();
        return (!value.is_empty()).then_some(value).into_iter().collect();
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn acoustic_material_library_from_ytyp(
    metadata: &serde_json::Value,
) -> Option<newengine_audio_api::AcousticMaterialLibrary> {
    let library = metadata.get("acoustic_material_library")?;
    let raw_materials = library.get("material")?;
    let materials = raw_materials
        .as_array()
        .map(|values| values.iter().collect::<Vec<_>>())
        .unwrap_or_else(|| vec![raw_materials]);
    let mut rules = Vec::new();
    for material in materials {
        let Some(material_id) = material.get("material_id").and_then(value_string) else {
            continue;
        };
        let Some(transmission_gain) = material.get("transmission_gain").and_then(value_f32) else {
            continue;
        };
        let reflection_gain = material
            .get("reflection_gain")
            .and_then(value_f32)
            .unwrap_or_else(|| {
                newengine_audio_api::AcousticMaterialProfile::default().reflection_gain
            });
        let Some(high_frequency_absorption) = material
            .get("high_frequency_absorption")
            .and_then(value_f32)
        else {
            continue;
        };
        let Some(low_pass_hz) = material.get("low_pass_hz").and_then(value_f32) else {
            continue;
        };
        let surface_matches = material
            .get("match")
            .map(string_or_array)
            .unwrap_or_default();
        if surface_matches.is_empty() {
            continue;
        }
        rules.push(newengine_audio_api::AcousticMaterialRule {
            material_id,
            surface_matches,
            profile: newengine_audio_api::AcousticMaterialProfile {
                transmission_gain,
                reflection_gain,
                high_frequency_absorption,
                low_pass_hz,
            },
        });
    }
    (!rules.is_empty()).then(|| newengine_audio_api::AcousticMaterialLibrary::new(rules))
}

fn merge_acoustic_material_library(
    target: &mut newengine_audio_api::AcousticMaterialLibrary,
    incoming: newengine_audio_api::AcousticMaterialLibrary,
) {
    for incoming_rule in incoming.rules {
        let incoming_matches = incoming_rule.surface_matches.clone();
        for rule in &mut target.rules {
            rule.surface_matches
                .retain(|pattern| !incoming_matches.iter().any(|value| value == pattern));
        }
        target.rules.retain(|rule| !rule.surface_matches.is_empty());
        target.rules.push(incoming_rule);
    }
    *target = target.clone().sanitized();
}
