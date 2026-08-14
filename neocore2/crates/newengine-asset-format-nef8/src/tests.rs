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

#[test]
fn published_nef8_content_kind_ids_are_unique() {
    let mut kinds = std::collections::BTreeMap::<u32, String>::new();
    for descriptor in descriptors() {
        let Some(kind) = descriptor.content_kind else {
            continue;
        };
        assert_ne!(kind, newengine_assets_api::LIST_FILE_CONTENT_KIND_UNKNOWN);
        if let Some(previous) = kinds.insert(kind, descriptor.extension.clone()) {
            panic!(
                "duplicate NEF8 content kind {} for .{} and .{}",
                kind, previous, descriptor.extension
            );
        }
    }
    assert_eq!(
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YBN,
        8,
        "legacy YBN id is frozen"
    );
    assert_eq!(
        newengine_assets_api::LIST_FILE_CONTENT_KIND_NEFTD,
        22,
        "NEFTD must have a dedicated non-colliding id"
    );
}

#[test]
fn ysc_is_a_selectorless_script_module_asset() {
    let descriptor = descriptor_for_extension("ysc").expect("ysc descriptor");
    assert_eq!(descriptor.extension, ysc::EXTENSION);
    assert_eq!(descriptor.asset_kind, ysc::ASSET_KIND);
    assert_eq!(descriptor.asset_kind, "script_module");
    assert_eq!(descriptor.selector_syntax, None);
}
