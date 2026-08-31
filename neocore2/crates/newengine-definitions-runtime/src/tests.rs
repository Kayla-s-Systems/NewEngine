use super::*;

#[test]
fn imperative_side_effect_fields_are_rejected() {
    let raw = RawDefinitionEntryV1 {
        name: "garage".to_owned(),
        metadata: BTreeMap::from([(
            "side_effects".to_owned(),
            serde_json::json!([{ "run_code": "spawnGarageHardcodedLogic()" }]),
        )]),
        ..Default::default()
    };
    assert!(collect_side_effects(&raw).is_err());
}

#[test]
fn declarative_side_effect_is_allowed() {
    let raw = RawDefinitionEntryV1 {
        name: "body".to_owned(),
        metadata: BTreeMap::from([(
            "side_effects".to_owned(),
            serde_json::json!([{ "domain": "engine.assets.models", "effect": "require_drawable", "target": "models/foo.ydd@body" }]),
        )]),
        ..Default::default()
    };
    let effects = collect_side_effects(&raw).unwrap();
    assert_eq!(effects[0].domain, "engine.assets.models");
}

#[test]
fn refs_are_classified_by_extension() {
    let raw = RawDefinitionEntryV1 {
        name: "body".to_owned(),
        dependencies: vec![
            AssetDependencyRecord::new(
                "models/foo.ydd@body",
                "drawable",
                "engine.assets.models",
                true,
            ),
            AssetDependencyRecord::new(
                "materials/foo.nemat@body",
                "material",
                "engine.assets.materials",
                true,
            ),
            AssetDependencyRecord::new(
                "textures/foo.ytd@diff",
                "texture",
                "engine.assets.textures",
                true,
            ),
        ],
        ..Default::default()
    };
    let refs = collect_refs(&raw);
    assert_eq!(refs.drawable_refs, vec!["models/foo.ydd@body"]);
    assert_eq!(refs.material_refs, vec!["materials/foo.nemat@body"]);
    assert_eq!(refs.texture_refs, vec!["textures/foo.ytd@diff"]);
}

#[test]
fn json_ytyp_dictionary_preserves_uv_layout_refs_and_arbitrary_strings() {
    let body = br#"{
            "schema": "newengine.ytyp.dictionary.v1",
            "entries": [
                {
                    "name": "sky_northstar_default",
                    "semantic_tags": ["sky"],
                    "dependencies": [
                        {
                            "reference": "layouts/sky.ytyd@skydome_uv",
                            "role": "uv_layout",
                            "domain": "engine.model",
                            "required": true
                        }
                    ],
                    "metadata": {
                        "newengine.game_ready": {
                            "sky": {
                                "mesh": "any authored mesh string",
                                "definition_ref": "any authored definition string"
                            }
                        },
                        "render": {
                            "role": "sky_background",
                            "uv.policy": "authored_ytyd"
                        }
                    }
                }
            ]
        }
        "#;
    let (entries, warnings) = parse_ytyp_json_document("definitions/sky.ytyp", body).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(warnings.iter().any(|warning| warning.contains("JSON")));
    let entry = build_entry("definitions/sky.ytyp", entries[0].clone(), &warnings).unwrap();
    assert_eq!(
        entry.refs.uv_layout_refs,
        vec!["layouts/sky.ytyd@skydome_uv"]
    );
    assert!(entry.model_explanation.render_options.is_sky_role());
    assert_eq!(entry.model_explanation.uv_policy, "authored_ytyd");
    let metadata = entry
        .arbitrary_metadata
        .get("metadata")
        .and_then(|value| value.get("newengine.game_ready"))
        .and_then(|value| value.get("sky"))
        .unwrap();
    assert_eq!(
        metadata.get("mesh").and_then(|value| value.as_str()),
        Some("any authored mesh string")
    );
}

#[test]
fn xml_ytyp_preserves_character_turn_in_place_attributes_in_game_ready_metadata() {
    let body = br#"<YtypProperties schema="newengine.ytyp.properties.v1" name="player_test" kind="game_ready_metadata">
        <Metadata>
            <Namespace name="newengine.game_ready">
                <player
                    model="models/characters/test/test.ydd@test"
                    idle_animation="animations/characters/test/mm.ycd@idle"
                    equipment_ready_animation="animations/characters/test/gun.ycd@ready"
                    turn_45_left_animation="animations/characters/test/mm.ycd@turn45l"
                    turn_90_right_animation="animations/characters/test/mm.ycd@turn90r"
                    turn_180_left_animation="animations/characters/test/mm.ycd@turn180l" />
            </Namespace>
        </Metadata>
    </YtypProperties>"#;
    let (entries, warnings) =
        parse_ytyp_xml_document("definitions/fps/player_test.ytyp", body).unwrap();
    let entry = build_entry(
        "definitions/fps/player_test.ytyp",
        entries[0].clone(),
        &warnings,
    )
    .unwrap();
    let player = entry
        .arbitrary_metadata
        .get("metadata")
        .and_then(|value| value.get("newengine.game_ready"))
        .and_then(|value| value.get("player"))
        .expect("player metadata");
    assert_eq!(
        player
            .get("turn_45_left_animation")
            .and_then(|value| value.as_str()),
        Some("animations/characters/test/mm.ycd@turn45l")
    );
    assert_eq!(
        player
            .get("turn_90_right_animation")
            .and_then(|value| value.as_str()),
        Some("animations/characters/test/mm.ycd@turn90r")
    );
    assert_eq!(
        player
            .get("turn_180_left_animation")
            .and_then(|value| value.as_str()),
        Some("animations/characters/test/mm.ycd@turn180l")
    );
}

#[test]
fn xml_ytyp_preserves_project_camera_definition_namespace() {
    let body = br#"<YtypProperties schema="newengine.ytyp.properties.v1" name="player_camera" kind="camera_definition">
        <Metadata>
            <Namespace name="newengine.camera">
                <camera schema="newengine.camera.definition.v1">
                    <first_person fov_y_degrees="71" hide_local_model="true" collision_probe_radius="0.061" />
                    <third_person collision_enabled="true">
                        <follow offset="0.4,1.7,5.2" zoom_min="1.5" zoom_max="11.0" />
                    </third_person>
                </camera>
            </Namespace>
        </Metadata>
    </YtypProperties>"#;
    let (entries, warnings) =
        parse_ytyp_xml_document("definitions/camera/player_camera.ytyp", body).unwrap();
    let entry = build_entry(
        "definitions/camera/player_camera.ytyp",
        entries[0].clone(),
        &warnings,
    )
    .unwrap();
    let camera = entry
        .arbitrary_metadata
        .get("metadata")
        .and_then(|value| value.get("newengine.camera"))
        .and_then(|value| value.get("camera"))
        .expect("camera metadata");
    assert_eq!(
        camera.get("schema").and_then(|value| value.as_str()),
        Some("newengine.camera.definition.v1")
    );
    let first_person = camera.get("first_person").expect("first_person");
    assert_eq!(
        first_person
            .get("fov_y_degrees")
            .and_then(|value| value.as_f64()),
        Some(71.0)
    );
    assert_eq!(
        first_person
            .get("hide_local_model")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    let offset = camera
        .get("third_person")
        .and_then(|value| value.get("follow"))
        .and_then(|value| value.get("offset"))
        .and_then(|value| value.as_array())
        .expect("follow offset");
    assert_eq!(offset.len(), 3);
    assert_eq!(offset[2].as_f64(), Some(5.2));
}

#[test]
fn logical_ref_normalization_is_linear_and_stable() {
    assert_eq!(
        normalize_logical_ref(r"  .\definitions\\fps//player.ytyp  "),
        "definitions/fps/player.ytyp"
    );
    assert_eq!(
        normalize_logical_ref("////definitions///fps///player.ytyp"),
        "definitions/fps/player.ytyp"
    );
    assert_eq!(
        normalize_logical_ref("../shared/player.ytyp"),
        "../shared/player.ytyp"
    );
}
