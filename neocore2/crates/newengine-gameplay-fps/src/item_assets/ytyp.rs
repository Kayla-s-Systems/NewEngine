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

pub(super) fn apply_weapon_ytyp_namespace(
    authored: &mut AuthoredItemDefinition,
    namespace: &serde_json::Value,
) -> Result<(), String> {
    if authored.kind.trim().eq_ignore_ascii_case("weapon") {
        let mut weapon = authored.weapon.take().unwrap_or_default();
        weapon.ammo = string(namespace, "weapon", "ammo")
            .ok_or_else(|| format!("weapon YTYP '{}' has no ammo", authored.definition_ref))?;
        weapon.fire_mode =
            string(namespace, "weapon", "fire_mode").unwrap_or_else(|| "semi_auto".to_owned());
        if let Some(value) = u32_value(namespace, "weapon", "magazine_capacity") {
            weapon.magazine_capacity = value;
        }
        if let Some(value) = u32_value(namespace, "weapon", "reserve_capacity") {
            weapon.reserve_capacity = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "fire_interval") {
            weapon.fire_interval = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "reload_duration") {
            weapon.reload_duration = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "damage") {
            weapon.damage = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "range") {
            weapon.range = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "hip_spread_degrees") {
            weapon.hip_spread_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "aim_spread_degrees") {
            weapon.aim_spread_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_pitch_degrees") {
            weapon.recoil_pitch_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "recoil_yaw_degrees") {
            weapon.recoil_yaw_degrees = value;
        }
        if let Some(value) = f32_value(namespace, "weapon", "muzzle_forward_offset") {
            weapon.muzzle_forward_offset = value;
        }
        // Validate the mode while the source reference is still available in the error.
        weapon
            .fire_mode()
            .map_err(|error| format!("weapon YTYP '{}': {error}", authored.definition_ref))?;
        authored.weapon = Some(weapon);

        let audio = AuthoredWeaponAudioDefinition {
            fire: string(namespace, "audio", "fire").unwrap_or_default(),
            reload_start: string(namespace, "audio", "reload_start").unwrap_or_default(),
            reload_complete: string(namespace, "audio", "reload_complete").unwrap_or_default(),
            equip: string(namespace, "audio", "equip").unwrap_or_default(),
            unequip: string(namespace, "audio", "unequip").unwrap_or_default(),
            empty: string(namespace, "audio", "empty").unwrap_or_default(),
            shell_eject: string(namespace, "audio", "shell_eject").unwrap_or_default(),
        };
        let has_audio = !audio.fire.trim().is_empty()
            || !audio.reload_start.trim().is_empty()
            || !audio.reload_complete.trim().is_empty()
            || !audio.equip.trim().is_empty()
            || !audio.unequip.trim().is_empty()
            || !audio.empty.trim().is_empty()
            || !audio.shell_eject.trim().is_empty();
        if has_audio {
            authored.weapon_audio = Some(audio);
        }
    }

    let mut world = authored.world.take().unwrap_or_default();
    if let Some(value) = string(namespace, "world", "model") {
        world.model = value;
    }
    if let Some(value) = string(namespace, "world", "material_library") {
        world.material_library = value;
    }
    if let Some(value) = f32_value(namespace, "world", "scale") {
        world.scale = [value; 3];
    }
    if let Some(value) = vec3(namespace, "world", "pickup_half_extents") {
        world.pickup_half_extents = value;
    }
    authored.world = Some(world);
    Ok(())
}

pub(crate) fn hydrate_item_package_from_ytyp(
    package: &mut AuthoredItemPackage,
) -> Result<usize, String> {
    let mut hydrated = 0usize;
    for authored in &mut package.items {
        let definition_ref = authored.definition_ref.trim().replace('\\', "/");
        if definition_ref.is_empty() {
            continue;
        }
        let payload = serde_json::to_vec(&serde_json::json!({
            "definition_ref": definition_ref,
        }))
        .map_err(|error| format!("weapon YTYP request encode failed: {error}"))?;
        let bytes = newengine_core::call_service_v1_optional(
            newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            newengine_assets_api::definitions_method::ENTRY_JSON_V1,
            &payload,
        )
        .map_err(|error| {
            format!(
                "weapon YTYP lookup failed item='{}' ref='{}': {error}",
                authored.id, definition_ref
            )
        })?
        .ok_or_else(|| {
            format!(
                "weapon YTYP definitions service unavailable item='{}' ref='{}'",
                authored.id, definition_ref
            )
        })?;
        let entry: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "weapon YTYP entry JSON invalid item='{}' ref='{}': {error}",
                authored.id, definition_ref
            )
        })?;
        let namespace = weapon_namespace(&entry).ok_or_else(|| {
            format!(
                "weapon YTYP has no newengine.weapon metadata item='{}' ref='{}'",
                authored.id, definition_ref
            )
        })?;
        apply_weapon_ytyp_namespace(authored, namespace)?;
        hydrated += 1;
    }
    Ok(hydrated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_ytyp_namespace_hydrates_fire_reload_and_world_presentation() {
        let mut item = AuthoredItemDefinition {
            id: "weapon.test".to_owned(),
            definition_ref: "definitions/weapon/test.ytyp@test".to_owned(),
            kind: "weapon".to_owned(),
            ..AuthoredItemDefinition::default()
        };
        let metadata = serde_json::json!({
            "weapon": {
                "ammo": "ammo.test",
                "fire_mode": "automatic",
                "magazine_capacity": 32,
                "reserve_capacity": 160,
                "fire_interval": 0.075,
                "reload_duration": 1.42,
                "damage": 19.0,
                "range": 87.0,
                "hip_spread_degrees": 2.0,
                "aim_spread_degrees": 0.4,
                "recoil_pitch_degrees": 0.9,
                "recoil_yaw_degrees": 0.3,
                "muzzle_forward_offset": 0.51
            },
            "audio": {
                "fire": "shared/audio/weapon/test/fire.wav",
                "reload_start": "shared/audio/weapon/test/reload.wav",
                "equip": "shared/audio/weapon/test/equip.wav",
                "empty": "shared/audio/weapon/test/empty.wav",
                "shell_eject": "shared/audio/weapon/test/shell.wav"
            },
            "world": {
                "model": "models/weapon/test.ydd@test",
                "material_library": "materials/test.nemat",
                "scale": 1.0,
                "pickup_half_extents": [0.2, 0.4, 0.1]
            }
        });
        apply_weapon_ytyp_namespace(&mut item, &metadata).expect("hydrate");
        let weapon = item.weapon.expect("weapon");
        assert_eq!(weapon.fire_mode().expect("mode"), WeaponFireMode::Automatic);
        assert_eq!(weapon.magazine_capacity, 32);
        assert!((weapon.reload_duration - 1.42).abs() < 1.0e-6);
        assert!((weapon.damage - 19.0).abs() < 1.0e-6);
        let audio = item.weapon_audio.expect("weapon audio");
        assert_eq!(audio.fire, "shared/audio/weapon/test/fire.wav");
        assert_eq!(audio.reload_start, "shared/audio/weapon/test/reload.wav");
        assert_eq!(audio.shell_eject, "shared/audio/weapon/test/shell.wav");
        let world = item.world.expect("world");
        assert_eq!(world.model, "models/weapon/test.ydd@test");
        assert_eq!(world.material_library, "materials/test.nemat");
        assert_eq!(world.pickup_half_extents, [0.2, 0.4, 0.1]);
    }
}
