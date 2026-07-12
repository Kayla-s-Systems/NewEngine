use super::*;

#[test]
fn native_key_names_resolve_through_one_canonical_table() {
    assert_eq!(
        key_identity::canonical_id_from_native_physical_name("KeyW"),
        Some(key_identity::KEY_W)
    );
    assert_eq!(
        key_identity::key_code_from_native_physical_name("KeyW"),
        Some(key_code::KEY_W)
    );
}

#[test]
fn engine_defaults_reference_canonical_key_codes() {
    assert_eq!(engine_default_keybind::PRIMARY_UI_TOGGLE, key_code::ESCAPE);
    assert_eq!(
        engine_default_keybind::ASSET_CATALOG_UI_TOGGLE,
        key_code::F1
    );
}

#[test]
fn snapshots_default_to_empty_provider_state() {
    let snapshot = InputStateSnapshot::default();
    assert!(snapshot.gamepads.is_empty());
}
