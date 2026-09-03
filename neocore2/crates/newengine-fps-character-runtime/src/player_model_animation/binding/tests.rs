use super::*;

#[test]
fn equipment_family_ready_phase_overrides_generic_phase_only_for_that_family() {
    let generic = EquipmentPoseSet::default();
    let pistol = EquipmentPoseSet {
        ready_sample_phase: Some(1.0),
        ..Default::default()
    };
    let mut families = std::collections::BTreeMap::new();
    families.insert("pistol".to_owned(), pistol);

    let pistol_set = select_equipment_pose_set(&generic, &families, Some("pistol"));
    let knife_set = select_equipment_pose_set(&generic, &families, Some("knife"));
    let unclassified_set = select_equipment_pose_set(&generic, &families, None);

    assert_eq!(
        equipment_ready_sample_phase_for_pose_set(pistol_set, 0.25),
        1.0
    );
    assert_eq!(
        equipment_ready_sample_phase_for_pose_set(knife_set, 0.25),
        0.25
    );
    assert_eq!(
        equipment_ready_sample_phase_for_pose_set(unclassified_set, 0.25),
        0.25
    );
}

#[test]
fn classified_equipment_family_never_falls_back_to_generic_pose_set() {
    let generic = EquipmentPoseSet::default();
    let mut families = std::collections::BTreeMap::new();

    assert!(select_equipment_pose_set(&generic, &families, None).is_some());
    assert!(select_equipment_pose_set(&generic, &families, Some("knife")).is_none());

    families.insert("knife".to_owned(), EquipmentPoseSet::default());
    assert!(select_equipment_pose_set(&generic, &families, Some("knife")).is_some());
    assert!(select_equipment_pose_set(&generic, &families, Some("bow")).is_none());
}
