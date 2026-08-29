use crate::references::{normalize_logical_ref, parse_texture_reference};

#[test]
fn rejects_raw_image_ref_before_vfs() {
    assert!(parse_texture_reference("textures/foo.png").is_err());
    assert!(parse_texture_reference("textures/foo.jpg").is_err());
    assert!(parse_texture_reference("textures/foo.dds").is_err());
}

#[test]
fn rejects_retired_texture_dictionary_ref_before_vfs() {
    assert!(parse_texture_reference("textures/foo.rawtex@bar").is_err());
}

#[test]
fn rejects_ytd_without_entry() {
    assert!(parse_texture_reference("textures/foo.ytd").is_err());
}

#[test]
fn accepts_ytd_entry_and_hash() {
    let named = parse_texture_reference("textures/foo.ytd@bar").unwrap();
    assert_eq!(named.texture_name.as_deref(), Some("bar"));
    assert_eq!(named.texture_hash, None);

    let hashed = parse_texture_reference("textures/foo.ytd@hash:123456").unwrap();
    assert_eq!(hashed.texture_name, None);
    assert_eq!(hashed.texture_hash, Some(123456));
}

#[test]
fn logical_ref_normalization_collapses_mixed_separators() {
    assert_eq!(
        normalize_logical_ref(r"./textures\\world//diffuse.ytd@base"),
        "textures/world/diffuse.ytd@base"
    );
}
