use super::*;

#[test]
fn default_policy_is_schema_only_and_requires_project_authored_content() {
    let policy = FpsGameplayPolicySnapshot::default();
    assert_eq!(policy.schema, FPS_GAMEPLAY_POLICY_SCHEMA);
    assert!(policy.characters.is_empty());
    assert!(policy.validate().is_err());
}

#[test]
fn character_animation_bindings_are_project_defined_not_directory_owned() {
    let character = FpsPlayableCharacterPolicy {
        id: "project_character".to_owned(),
        family: "Project".to_owned(),
        display_name: "Project Character".to_owned(),
        runtime_ready: true,
        runtime_model_ref: Some("arbitrary/storage/avatar.asset@body".to_owned()),
        skeleton_ref: Some("rigs/shared/runtime.asset@skeleton".to_owned()),
        animations: FpsPlayableCharacterAnimations {
            slots: BTreeMap::from([
                (
                    "locomotion.rest".to_owned(),
                    "motion/banks/a.asset@rest".to_owned(),
                ),
                (
                    "project.inspect.head".to_owned(),
                    "anywhere/custom.bin@head_turn".to_owned(),
                ),
            ]),
            idle: Some("legacy/location/also_allowed.asset@idle".to_owned()),
            ..FpsPlayableCharacterAnimations::default()
        },
        target_height: 1.70,
        ..FpsPlayableCharacterPolicy::default()
    };
    character.validate().expect(
        "animation references are opaque project-authored bindings, not directory ownership",
    );
}

#[test]
fn arbitrary_animation_slot_rejects_blank_binding_but_not_custom_names() {
    let mut character = FpsPlayableCharacterPolicy {
        id: "custom".to_owned(),
        family: "Project".to_owned(),
        display_name: "Custom".to_owned(),
        animations: FpsPlayableCharacterAnimations {
            slots: BTreeMap::from([(
                "my.gameplay.mode.experimental".to_owned(),
                "custom/protocol/ref#42".to_owned(),
            )]),
            ..FpsPlayableCharacterAnimations::default()
        },
        ..FpsPlayableCharacterPolicy::default()
    };
    character
        .validate()
        .expect("custom slot names and refs are project data");
    character
        .animations
        .slots
        .insert("broken".to_owned(), "   ".to_owned());
    assert!(character.validate().is_err());
}

#[test]
fn character_menu_does_not_require_equipment_ik_rig() {
    let character = FpsPlayableCharacterPolicy {
        id: "generic_entity".to_owned(),
        family: "Test".to_owned(),
        display_name: "Generic Entity".to_owned(),
        presentation: FpsCharacterPresentationPolicy {
            equipment_arm_ik: true,
            equipment_arm_ik_rig: None,
            ..FpsCharacterPresentationPolicy::default()
        },
        ..FpsPlayableCharacterPolicy::default()
    };
    let policy = FpsCharacterMenuPolicySnapshot {
        characters: vec![character],
        ..FpsCharacterMenuPolicySnapshot::default()
    };
    policy
        .validate()
        .expect("optional equipment IK must not invalidate Character Menu");
}

#[test]
fn runtime_character_admission_does_not_require_equipment_ik_rig() {
    let character = FpsPlayableCharacterPolicy {
        id: "generic_entity".to_owned(),
        family: "Test".to_owned(),
        display_name: "Generic Entity".to_owned(),
        runtime_ready: true,
        runtime_model_ref: Some("models/test/generic.ydd@generic".to_owned()),
        target_height: 1.8,
        presentation: FpsCharacterPresentationPolicy {
            equipment_arm_ik: true,
            equipment_arm_ik_rig: None,
            ..FpsCharacterPresentationPolicy::default()
        },
        ..FpsPlayableCharacterPolicy::default()
    };
    character
        .validate()
        .expect("optional equipment IK must not reject a valid visual entity");
}

#[test]
fn callback_export_is_not_a_ysc_selector() {
    let callbacks = FpsCallbackExports {
        interaction: "on_interaction".to_owned(),
        hit: "scripts/foo.ysc@on_hit".to_owned(),
        mission_event: "on_mission_event".to_owned(),
    };
    assert!(callbacks.validate().is_err());
}

#[test]
fn callback_damage_multiplier_must_be_finite() {
    let decision = FpsPolicyDecision {
        damage_multiplier: f32::NAN,
        ..FpsPolicyDecision::default()
    };
    assert!(decision.validate().is_err());
}

#[test]
fn character_menu_policy_validates_semantic_toggle_contract() {
    let mut policy = FpsCharacterMenuPolicySnapshot::default();
    policy.title = "MODEL".to_owned();
    policy.validate().expect("default semantic menu policy");

    policy.toggle_action = "KeyM".to_owned();
    policy
        .validate()
        .expect("policy accepts provider-authored semantic action ids");

    policy.toggle_action = "bad action with spaces".to_owned();
    assert!(policy.validate().is_err());
}
