use super::*;

#[test]
fn camera_defaults_never_masquerade_as_an_authored_project_camera() {
    let spec = crate::content::GameReadyCameraSpec::default();
    assert!(!spec.declared);
    assert!(spec.definition_ref.is_empty());
    assert!(spec.instance_id.is_empty());
}

#[test]
fn complete_motion_response_block_is_typed_without_invented_fields() {
    let player = serde_json::json!({
        "motion_response": {
            "velocity_spring_const": 7.0,
            "velocity_spring_const_decel": 10.0,
            "velocity_spring_dampen_ratio": 1.0,
            "speed_spring_const": 4.6,
            "max_accel": -1.0,
            "trans_clamp_dist": 0.01
        }
    });
    let response = player_motion_response_from_ytyp(&player).expect("typed response");
    assert_eq!(response.velocity_spring_const, 7.0);
    assert_eq!(response.velocity_spring_const_decel, 10.0);
    assert_eq!(response.velocity_spring_dampen_ratio, 1.0);
    assert_eq!(response.speed_spring_const, 4.6);
    assert_eq!(response.max_accel, -1.0);
    assert_eq!(response.trans_clamp_dist, 0.01);
}

#[test]
fn partial_motion_response_block_is_rejected_instead_of_filling_guesses() {
    let player = serde_json::json!({
        "motion_response": {
            "velocity_spring_const": 7.0,
            "velocity_spring_const_decel": 10.0
        }
    });
    assert!(player_motion_response_from_ytyp(&player).is_none());
}

#[test]
fn equipment_animation_family_attributes_are_open_ended() {
    assert_eq!(
        player::equipment_animation_slot_from_attribute("equipment_knife_ready_animation")
            .as_deref(),
        Some("equipment.knife.ready")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute("equipment_pistol_reload_animation")
            .as_deref(),
        Some("equipment.pistol.reload")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute("equipment_long_gun_aim_animation")
            .as_deref(),
        Some("equipment.long_gun.aim")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute("equipment_modded-family_ready_animation")
            .as_deref(),
        Some("equipment.modded_family.ready")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute("equipment_rifle_aim_move_fw45l_animation")
            .as_deref(),
        Some("equipment.rifle.aim.move.fw45l")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute(
            "equipment_long_gun_crouch_aim_move_b135r_animation"
        )
        .as_deref(),
        Some("equipment.long_gun.crouch.aim.move.b135r")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute(
            "equipment_rifle_grip_stand_hands_animation"
        )
        .as_deref(),
        Some("equipment.rifle.grip.stand.hands")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute(
            "equipment_rifle_grip_stand_fingers_animation"
        )
        .as_deref(),
        Some("equipment.rifle.grip.stand.fingers")
    );
    assert_eq!(
        player::equipment_animation_slot_from_attribute(
            "equipment_rifle_transition_ready_to_aim_animation"
        )
        .as_deref(),
        Some("equipment.rifle.transition.ready_to_aim")
    );
}

#[test]
fn equipment_ready_sample_phase_family_attributes_are_open_ended() {
    assert_eq!(
        player::equipment_ready_sample_phase_family_from_attribute(
            "equipment_pistol_ready_sample_phase"
        )
        .as_deref(),
        Some("pistol")
    );
    assert_eq!(
        player::equipment_ready_sample_phase_family_from_attribute(
            "equipment_long-gun_ready_sample_phase"
        )
        .as_deref(),
        Some("long_gun")
    );
    assert!(
        player::equipment_ready_sample_phase_family_from_attribute("equipment_ready_sample_phase")
            .is_none()
    );
    assert!(
        player::equipment_ready_sample_phase_family_from_attribute(
            "equipment_pistol_aim_sample_phase"
        )
        .is_none()
    );
}

#[test]
fn generic_or_malformed_equipment_attributes_do_not_become_family_slots() {
    assert!(player::equipment_animation_slot_from_attribute("equipment_ready_animation").is_none());
    assert!(
        player::equipment_animation_slot_from_attribute("equipment__ready_animation").is_none()
    );
    assert!(
        player::equipment_animation_slot_from_attribute("equipment_knife_attack_animation")
            .is_none()
    );
    assert!(
        player::equipment_animation_slot_from_attribute("look_knife_ready_animation").is_none()
    );
}

#[test]
fn compact_equipment_rotation_weights_parse_from_ytyp_attribute() {
    let value = serde_json::Value::String("spineb:0.22;r_shoulder:0.92;r_palm:1.0".to_owned());
    let weights = player_joint_rotation_weights(&value).expect("weights");
    assert_eq!(weights.len(), 3);
    assert_eq!(weights[0].joint, "spineb");
    assert!((weights[0].weight - 0.22).abs() < 1.0e-6);
    assert_eq!(weights[1].joint, "r_shoulder");
    assert!((weights[1].weight - 0.92).abs() < 1.0e-6);
    assert_eq!(weights[2].joint, "r_palm");
    assert!((weights[2].weight - 1.0).abs() < 1.0e-6);
}
#[test]
fn acoustic_material_library_hydrates_from_definition_metadata_projection() {
    let metadata = serde_json::json!({
        "acoustic_material_library": {
            "schema": "newengine.audio.acoustic-material-library.v2",
            "version": 2,
            "material": [
                {
                    "material_id": "material.test.a",
                    "transmission_gain": 0.25,
                    "reflection_gain": 0.72,
                    "high_frequency_absorption": 0.75,
                    "low_pass_hz": 2400.0,
                    "match": "solid_a"
                },
                {
                    "material_id": "material.test.b",
                    "transmission_gain": 0.55,
                    "high_frequency_absorption": 0.40,
                    "low_pass_hz": 6400.0,
                    "match": ["panel_b", "sheet_b"]
                }
            ]
        }
    });
    let library = acoustic_material_library_from_ytyp(&metadata).expect("acoustic library");
    assert_eq!(library.rules.len(), 2);
    assert_eq!(
        library.resolve("surface.wall.solid_a").unwrap().material_id,
        "material.test.a"
    );
    assert_eq!(
        library.resolve("surface.sheet_b").unwrap().material_id,
        "material.test.b"
    );
    assert!(
        (library
            .resolve("surface.wall.solid_a")
            .unwrap()
            .profile
            .reflection_gain
            - 0.72)
            .abs()
            < 1.0e-6
    );
    assert!(
        (library
            .resolve("surface.sheet_b")
            .unwrap()
            .profile
            .reflection_gain
            - newengine_audio_api::AcousticMaterialProfile::default().reflection_gain)
            .abs()
            < 1.0e-6
    );
}

#[test]
fn later_acoustic_library_replaces_matching_shared_rule_only() {
    let mut shared = newengine_audio_api::AcousticMaterialLibrary::new(vec![
        newengine_audio_api::AcousticMaterialRule {
            material_id: "material.shared.wall".to_owned(),
            surface_matches: vec!["wall".to_owned(), "masonry".to_owned()],
            profile: newengine_audio_api::AcousticMaterialProfile::default(),
        },
    ]);
    let project = newengine_audio_api::AcousticMaterialLibrary::new(vec![
        newengine_audio_api::AcousticMaterialRule {
            material_id: "material.project.wall".to_owned(),
            surface_matches: vec!["wall".to_owned()],
            profile: newengine_audio_api::AcousticMaterialProfile {
                transmission_gain: 0.9,
                reflection_gain: 0.2,
                high_frequency_absorption: 0.1,
                low_pass_hz: 12_000.0,
            },
        },
    ]);
    merge_acoustic_material_library(&mut shared, project);
    assert_eq!(
        shared.resolve("surface.wall").unwrap().material_id,
        "material.project.wall"
    );
    assert_eq!(
        shared.resolve("surface.masonry").unwrap().material_id,
        "material.shared.wall"
    );
}
