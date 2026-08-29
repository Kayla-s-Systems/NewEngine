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

#[test]
fn content_kind_lookup_and_default_routes_are_canonical() {
    let spec = spec_for_content_kind(ytyp::CONTENT_KIND).expect("YTYP spec by content kind");
    assert_eq!(spec.extension, ytyp::EXTENSION);
    assert_eq!(spec.handler_service, ytyp::HANDLER_SERVICE);

    let route =
        default_entry_route_for_content_kind(ytyp::CONTENT_KIND).expect("YTYP default entry route");
    assert_eq!(
        route.gateway,
        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID
    );
    assert_eq!(
        route.method,
        newengine_assets_api::definitions_method::ENTRY_JSON_V1
    );
    assert_eq!(route.semantic_owner, "definition");

    assert!(spec_for_content_kind(0).is_none());
    assert!(default_entry_route_for_content_kind(0).is_none());
}

#[test]
fn yft_is_registered_and_keeps_frozen_content_kind() {
    let descriptor = descriptor_for_extension("yft").expect("YFT descriptor");
    assert_eq!(
        descriptor.content_kind,
        Some(newengine_assets_api::LIST_FILE_CONTENT_KIND_YFT)
    );
    assert_eq!(descriptor.content_kind, Some(7));
    assert_eq!(descriptor.semantic_gateway, "engine.model");
}

#[test]
fn registry_covers_every_published_content_kind_exactly_once() {
    let registered = specs()
        .iter()
        .filter_map(|spec| spec.content_kind)
        .collect::<std::collections::BTreeSet<_>>();
    let published = newengine_assets_api::LIST_FILE_PUBLISHED_CONTENT_KINDS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        registered, published,
        "newengine-asset-format-nef8 must cover the complete engine-owned content-kind contract"
    );
    assert_eq!(
        registered.len(),
        newengine_assets_api::LIST_FILE_PUBLISHED_CONTENT_KINDS.len(),
        "published content-kind ids must be unique"
    );
}

#[test]
fn yscd_is_registered_as_embedded_sound_cue_dictionary() {
    let descriptor = descriptor_for_extension("yscd").expect("YSCD descriptor");
    assert_eq!(descriptor.content_kind, Some(34));
    assert_eq!(descriptor.asset_kind, "sound_cue_dictionary");
    assert_eq!(descriptor.semantic_gateway, "engine.audio");
    assert_eq!(descriptor.selector_syntax.as_deref(), Some("file.yscd@cue"));
}
