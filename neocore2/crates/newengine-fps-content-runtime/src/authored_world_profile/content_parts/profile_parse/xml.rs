use super::*;

use super::super::ymap_read_diagnostics::log_loaded_profile_summary;
use newengine_authored_xml as authored_xml;

pub(super) fn parse_ymap_xml_payload(
    payload: &[u8],
    logical_path: &str,
) -> Result<serde_json::Value, String> {
    let text = std::str::from_utf8(payload)
        .map_err(|e| format!("ymap XML body is not UTF-8 path='{logical_path}' err='{e}'"))?;
    let doc = authored_xml::parse_xml_document(text, &format!("ymap path='{logical_path}'"))?;
    let root = doc.root_element();
    if !root.has_tag_name("YmapMapDefinition") && !root.has_tag_name("MapDefinition") {
        return Err(format!(
            "ymap XML root must be <YmapMapDefinition> path='{logical_path}' actual='{}'",
            root.tag_name().name()
        ));
    }
    let schema = root.attribute("schema").unwrap_or_default();
    if !schema.starts_with("newengine.map.definition.") {
        return Err(format!("ymap unsupported XML schema path='{logical_path}' schema='{schema}' expected='newengine.map.definition.*'"));
    }
    let child_elements = root.children().filter(|child| child.is_element()).count();
    newengine_ulog_api::ulog::info!(
        "authored-world ymap read: XML accepted path='{}' payload_bytes={} root='{}' schema='{}' child_elements={}",
        logical_path,
        payload.len(),
        root.tag_name().name(),
        schema,
        child_elements,
    );

    let mut root_json = serde_json::Map::new();
    root_json.insert(
        "schema".to_owned(),
        serde_json::Value::String(schema.to_owned()),
    );
    if let Some(map_node) =
        authored_xml::xml_child(root, "map").or_else(|| authored_xml::xml_child(root, "Map"))
    {
        root_json.insert("map".to_owned(), ymap_node_object(map_node));
    } else if let Some(profile_node) = authored_xml::xml_child(root, "profile")
        .or_else(|| authored_xml::xml_child(root, "Profile"))
    {
        root_json.insert("profile".to_owned(), ymap_node_object(profile_node));
    } else {
        return Err(format!(
            "ymap XML has no <map> or <profile> node path='{logical_path}'"
        ));
    }
    Ok(serde_json::Value::Object(root_json))
}

pub(super) fn parse_map_definition_payload(
    value: serde_json::Value,
    logical_path: &str,
) -> Result<AuthoredWorldProfile, String> {
    let schema = value
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !schema.is_empty() && !schema.starts_with("newengine.map.definition.") {
        return Err(format!(
            "ymap unsupported schema path='{logical_path}' schema='{schema}' expected='newengine.map.definition.*'"
        ));
    }

    if let Some(profile) = value.pointer("/map/profile").cloned() {
        return parse_payload(profile, "ymap.map.profile", logical_path);
    }
    if let Some(profile) = value.get("profile").cloned() {
        return parse_payload(profile, "ymap.profile", logical_path);
    }
    if value.get("scene").is_some() {
        return Err(format!(
            "ymap scene payload rejected path='{logical_path}' policy='use newengine.map.definition.* with map.profile / profile / payload'"
        ));
    }
    if let Some(payload) = value.get("payload").cloned() {
        return parse_payload(payload, "ymap.payload", logical_path);
    }
    parse_payload(value, "ymap.root", logical_path)
}

pub(super) fn parse_payload(
    value: serde_json::Value,
    source_label: &str,
    logical_path: &str,
) -> Result<AuthoredWorldProfile, String> {
    let raw: RawAuthoredWorldPayload = serde_json::from_value(value)
        .map_err(|e| format!("map payload parse failed source='{source_label}': {e}"))?;
    let mut profile = raw.into_profile();
    for prefab in &mut profile.prefabs {
        prefab.authored_map_ref = logical_path.to_owned();
    }
    log_loaded_profile_summary(logical_path, source_label, &profile);
    Ok(profile)
}

fn ymap_node_object(node: authored_xml::XmlNode<'_, '_>) -> serde_json::Value {
    let tag = node.tag_name().name();
    if tag.eq_ignore_ascii_case("definition_refs") {
        let refs = node
            .children()
            .filter(|child| child.is_element())
            .filter_map(|child| {
                child
                    .attribute("value")
                    .or_else(|| child.attribute("ref"))
                    .map(str::trim)
            })
            .filter(|value| !value.is_empty())
            .map(|value| serde_json::Value::String(value.to_owned()))
            .collect::<Vec<_>>();
        return serde_json::Value::Array(refs);
    }
    if tag.eq_ignore_ascii_case("definitions")
        || tag.eq_ignore_ascii_case("placements")
        || tag.eq_ignore_ascii_case("prefabs")
        || tag.eq_ignore_ascii_case("policy")
        || tag.eq_ignore_ascii_case("layers")
        || tag.eq_ignore_ascii_case("surface_layers")
        || tag.eq_ignore_ascii_case("pickups")
        || tag.eq_ignore_ascii_case("targets")
        || tag.eq_ignore_ascii_case("hazards")
        || tag.eq_ignore_ascii_case("goals")
        || tag.eq_ignore_ascii_case("emitters")
    {
        let items = node
            .children()
            .filter(|child| child.is_element())
            .map(ymap_node_object)
            .collect::<Vec<_>>();
        return serde_json::Value::Array(items);
    }
    if tag.eq_ignore_ascii_case("DefinitionRef")
        || tag.eq_ignore_ascii_case("Policy")
        || tag.eq_ignore_ascii_case("Item")
    {
        if let Some(value) = node.attribute("value").or_else(|| node.attribute("ref")) {
            return authored_xml::xml_scalar(value);
        }
    }

    let element_children = node.children().filter(|child| child.is_element()).count();
    if element_children == 0 {
        let attr_count = node.attributes().count();
        if attr_count == 1 {
            if let Some(value) = node.attribute("value") {
                return authored_xml::xml_scalar(value);
            }
        }
    }

    let mut map = serde_json::Map::new();
    for attr in node.attributes() {
        map.insert(
            attr.name().to_owned(),
            authored_xml::xml_scalar(attr.value()),
        );
    }
    for child in node.children().filter(|child| child.is_element()) {
        let key = child.tag_name().name();
        let value = ymap_node_object(child);
        ymap_insert_child(&mut map, key, value);
    }
    if map.is_empty() {
        if let Some(text) = node.text().map(str::trim).filter(|text| !text.is_empty()) {
            return authored_xml::xml_scalar(text);
        }
    }
    serde_json::Value::Object(map)
}

fn ymap_insert_child(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: serde_json::Value,
) {
    let key = match key {
        "Definition" => "definitions",
        "Placement" => "placements",
        "Prefab" => "prefabs",
        "Layer" | "SurfaceLayer" => "layers",
        "Pickup" => "pickups",
        "Target" => "targets",
        "Hazard" => "hazards",
        "Goal" => "goals",
        "Emitter" => "emitters",
        other => other,
    };
    match map.get_mut(key) {
        Some(serde_json::Value::Array(items)) => items.push(value),
        Some(existing) => {
            let old = std::mem::replace(existing, serde_json::Value::Null);
            *existing = serde_json::Value::Array(vec![old, value]);
        }
        None => {
            map.insert(key.to_owned(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymap_enemy_ai_block_projects_into_typed_target_policy() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<YmapMapDefinition schema="newengine.map.definition.v2">
  <map id="ai-test">
    <profile title="AI test" objective="Observe target">
      <gameplay>
        <mission target_material="materials/test.nemat@target">
          <targets>
            <Target id="dummy" character_ref="definitions/fps/player_joel.ytyp@player_joel"
                    position="0,0,3" health="100" scale="0.55,1.05,0.55"
                    ai_enabled="true" combat_team="2" sight_range="24"
                    field_of_view_degrees="110" memory_seconds="3"
                    decision_interval_seconds="0.1" move_speed="2.4"
                    patrol_route="-2,0,-4;2,0,-4" patrol_looping="true"
                    investigate_arrival_distance="0.8" engage_standoff_distance="8.0"
                    waypoint_arrival_distance="0.35" repath_interval_seconds="0.35"
                    view_turn_speed_degrees_per_second="240" fire_distance="22"
                    aim_tolerance_degrees="3" weapon_muzzle_offset="0.20,1.20,-0.45"
                    weapon_muzzle_forward="0,0,-1" loadout="loadout.fps.default" />
          </targets>
        </mission>
      </gameplay>
    </profile>
  </map>
</YmapMapDefinition>"#;
        let value = parse_ymap_xml_payload(xml.as_bytes(), "maps/ai-test.ymap")
            .expect("parse authored YMAP XML");
        let profile = parse_map_definition_payload(value, "maps/ai-test.ymap")
            .expect("project typed YMAP profile");
        let target = profile
            .gameplay
            .mission
            .targets
            .first()
            .expect("authored target");
        assert_eq!(
            target.character_ref.as_deref(),
            Some("definitions/fps/player_joel.ytyp@player_joel")
        );
        let ai = target.ai.as_ref().expect("authored AI policy");
        assert_eq!(ai.combat_team, 2);
        assert_eq!(ai.sight_range, 24.0);
        assert_eq!(ai.field_of_view_degrees, 110.0);
        assert_eq!(ai.memory_seconds, 3.0);
        assert_eq!(ai.decision_interval_seconds, 0.1);
        assert_eq!(ai.navigation.move_speed, 2.4);
        assert_eq!(ai.patrol_route.len(), 2);
        assert_eq!(ai.patrol_route[0], Vec3::new(-2.0, 0.0, -4.0));
        assert!(ai.patrol_looping);
        assert_eq!(ai.navigation.engage_standoff_distance, 8.0);
        assert_eq!(ai.navigation.repath_interval_seconds, 0.35);
        assert!(
            (ai.navigation.view_turn_speed_radians_per_second - 240.0_f32.to_radians()).abs()
                < 1.0e-6
        );
        assert_eq!(ai.combat.fire_distance, 22.0);
        assert!((ai.combat.aim_tolerance_radians - 3.0_f32.to_radians()).abs() < 1.0e-6);
        assert_eq!(ai.weapon_mount.local_offset, [0.20, 1.20, -0.45]);
        assert_eq!(ai.weapon_mount.local_forward, [0.0, 0.0, -1.0]);
        assert_eq!(ai.loadout, "loadout.fps.default");
    }

    #[test]
    fn incomplete_enabled_enemy_ai_block_fails_closed() {
        let xml = r#"<YmapMapDefinition schema="newengine.map.definition.v2">
  <map id="ai-test"><profile><gameplay><mission><targets>
    <Target id="dummy" position="0,0,3" health="100" scale="1,1,1"
            ai_enabled="true" combat_team="2" />
  </targets></mission></gameplay></profile></map>
</YmapMapDefinition>"#;
        let value = parse_ymap_xml_payload(xml.as_bytes(), "maps/ai-invalid.ymap")
            .expect("parse authored XML envelope");
        let profile = parse_map_definition_payload(value, "maps/ai-invalid.ymap")
            .expect("malformed AI target is sanitized out rather than defaulted");
        assert!(profile.gameplay.mission.targets.is_empty());
    }

    #[test]
    fn ymap_audio_emitters_project_to_native_audio_components() {
        let xml = r#"<YmapMapDefinition schema="newengine.map.definition.v2">
  <map id="audio-test"><profile><audio><emitters>
    <Emitter id="room_bed" source="audio/ambience/room/room_tone.xvag"
             position="0,2,-3" gain="0.16" looping="true" spatial="false" occlusion_enabled="false" />
    <Emitter id="air_leak" source="audio/ambience/room/wind_window.xvag"
             position="4.5,2.2,-3" gain="0.10" looping="true" spatial="true"
             attenuation_min_distance="1.25" attenuation_max_distance="14.0"
             attenuation_curve="inverse" attenuation_rolloff="0.78" occlusion_enabled="true" />
  </emitters></audio></profile></map>
</YmapMapDefinition>"#;
        let value = parse_ymap_xml_payload(xml.as_bytes(), "maps/audio-test.ymap")
            .expect("parse authored XML audio");
        let profile = parse_map_definition_payload(value, "maps/audio-test.ymap")
            .expect("project typed audio profile");
        assert_eq!(profile.audio_emitters.len(), 2);
        let bed = &profile.audio_emitters[0];
        assert_eq!(bed.id, "room_bed");
        assert_eq!(bed.position, Vec3::new(0.0, 2.0, -3.0));
        assert!(!bed.emitter.spatial);
        assert!(!bed.emitter.occlusion.enabled);
        assert!((bed.emitter.gain - 0.16).abs() < 1.0e-6);
        let leak = &profile.audio_emitters[1];
        assert!(leak.emitter.spatial);
        assert!(leak.emitter.looping);
        assert!(leak.emitter.occlusion.enabled);
        assert_eq!(leak.emitter.source, "audio/ambience/room/wind_window.xvag");
        let attenuation = leak
            .emitter
            .attenuation
            .as_ref()
            .expect("native XVAG attenuation");
        assert!((attenuation.min_distance - 1.25).abs() < 1.0e-6);
        assert!((attenuation.max_distance - 14.0).abs() < 1.0e-6);
        assert!((attenuation.rolloff - 0.78).abs() < 1.0e-6);
    }
}
