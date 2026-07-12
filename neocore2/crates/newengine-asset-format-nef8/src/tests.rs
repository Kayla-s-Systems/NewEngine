use super::*;

#[test]
fn descriptors_are_unique_and_valid() {
    let descriptors = descriptors();
    let mut extensions = std::collections::BTreeSet::new();
    assert!(descriptors.len() >= 20);
    for descriptor in descriptors {
        assert!(
            extensions.insert(descriptor.extension.clone()),
            "duplicate extension {}",
            descriptor.extension
        );
        if descriptor.extension != nepak::EXTENSION {
            assert!(
                descriptor.validate_generic_rules().is_ok(),
                "invalid descriptor {}",
                descriptor.extension
            );
        }
    }
}

#[test]
fn ytd_descriptor_still_routes_to_texture_domain() {
    let descriptor = descriptor_for_extension("ytd").expect("ytd descriptor");
    assert_eq!(descriptor.extension, ytd::EXTENSION);
    assert_eq!(descriptor.content_kind, Some(ytd::CONTENT_KIND));
    assert_eq!(descriptor.semantic_gateway, ytd::SEMANTIC_GATEWAY);
}

#[test]
fn ytyd_descriptor_routes_to_model_domain() {
    let descriptor = descriptor_for_extension("ytyd").expect("ytyd descriptor");
    assert_eq!(descriptor.extension, ytyd::EXTENSION);
    assert_eq!(descriptor.content_kind, Some(ytyd::CONTENT_KIND));
    assert_eq!(descriptor.semantic_gateway, ytyd::SEMANTIC_GATEWAY);
    assert!(descriptor.schema_editable);
}

#[test]
fn extension_lookup_normalizes_dot_and_case_once() {
    let descriptor = descriptor_for_extension(".YDD").expect("normalized ydd descriptor");
    assert_eq!(descriptor.extension, ydd::EXTENSION);
}
