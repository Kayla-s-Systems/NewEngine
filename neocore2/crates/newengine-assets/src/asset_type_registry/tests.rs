use super::*;

fn explicit_descriptor(
    extension: &str,
    priority: i32,
    semantic_gateway: &str,
) -> AssetFileTypeDescriptor {
    AssetFileTypeDescriptor {
        module_id: format!("test.format.{extension}"),
        extension: extension.to_owned(),
        asset_kind: "provider_declared_asset".to_owned(),
        container: format!("newengine.listfile.nef8.{extension}"),
        content_kind: Some(1000),
        codec_type: newengine_assets_api::codec_type::LIST_FILE.to_owned(),
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
        semantic_gateway: semantic_gateway.to_owned(),
        handler_service: format!("asset.codec.listfile.{extension}"),
        selector_syntax: Some(format!("file.{extension}@entry")),
        consumer_domains: vec![semantic_gateway.to_owned()],
        magic: Some("4e454638".to_owned()),
        outputs: vec![
            newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
            "asset.blob".to_owned(),
        ],
        priority,
        vfs_backed: true,
        runtime_ready: true,
        allow_nested_assets: false,
        native_container: true,
        requires_magic: true,
        notes: "test descriptor declared by test format crate".to_owned(),
        ..Default::default()
    }
}

fn register_one(descriptor: AssetFileTypeDescriptor) -> AssetFileTypeDescriptor {
    let mut state = AssetTypeRegistryState::default();
    state.register(AssetFileTypeRegisterRequest { descriptor })
}

#[test]
fn registry_starts_empty_until_formats_self_register() {
    let state = AssetTypeRegistryState::default();
    assert!(state.manifest().formats.is_empty());
}

#[test]
fn registry_accepts_provider_declared_format_without_known_extension_or_gateway_branch() {
    let registered = register_one(explicit_descriptor("zzx", 0, "engine.assets.zzx"));
    assert_eq!(registered.extension, "zzx");
    assert_eq!(registered.semantic_gateway, "engine.assets.zzx");
    assert_eq!(registered.gateway, "engine.assets.zzx");
    assert_eq!(registered.content_kind, Some(1000));
    assert_eq!(
        newengine_service_api::service_kind_from_engine_gateway_id("engine.assets.zzx").as_deref(),
        Some("assets.zzx")
    );
}

#[test]
fn registry_rejects_descriptor_without_self_declared_semantic_gateway() {
    let registered = register_one(AssetFileTypeDescriptor {
        extension: "bad".to_owned(),
        asset_kind: "provider_declared_asset".to_owned(),
        codec_type: newengine_assets_api::codec_type::LIST_FILE.to_owned(),
        handler_service: "asset.codec.listfile.bad".to_owned(),
        magic: Some("4e454638".to_owned()),
        ..Default::default()
    });
    assert!(registered.notes.contains("descriptor rejected"));
}

#[test]
fn registry_uses_priority_for_same_extension_without_extension_specific_logic() {
    let mut state = AssetTypeRegistryState::default();
    let low = explicit_descriptor("same", 0, "engine.assets.low");
    let high = explicit_descriptor("same", 10, "engine.assets.high");
    state.register(AssetFileTypeRegisterRequest { descriptor: low });
    let registered = state.register(AssetFileTypeRegisterRequest { descriptor: high });
    assert_eq!(registered.semantic_gateway, "engine.assets.high");
    assert_eq!(
        state
            .probe(AssetFileTypeProbeRequest {
                logical_path: "foo.same@main".to_owned()
            })
            .descriptor
            .unwrap()
            .semantic_gateway,
        "engine.assets.high"
    );
}

#[test]
fn registry_keeps_container_semantics_generic() {
    let registered = register_one(AssetFileTypeDescriptor {
        extension: "pkgx".to_owned(),
        asset_kind: "asset_package".to_owned(),
        container: "newengine.asset_package.provider_declared".to_owned(),
        codec_type: newengine_assets_api::codec_type::CONTAINER.to_owned(),
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
        semantic_gateway: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
        handler_service: "asset.codec.pkgx".to_owned(),
        magic: Some("4e4550414b010000".to_owned()),
        consumer_domains: vec![newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned()],
        allow_nested_assets: true,
        native_container: true,
        runtime_ready: true,
        requires_magic: true,
        ..Default::default()
    });
    assert_eq!(
        registered.semantic_gateway,
        newengine_assets_api::ENGINE_ASSET_SERVICE_ID
    );
    assert_eq!(
        registered.consumer_domains,
        vec![newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned()]
    );
    assert!(registered.selector_syntax.is_none());
}

#[test]
fn registry_prefers_longest_registered_extension_suffix() {
    let mut state = AssetTypeRegistryState::default();
    state.register(AssetFileTypeRegisterRequest {
        descriptor: explicit_descriptor("map", 0, "engine.assets.short_map"),
    });
    state.register(AssetFileTypeRegisterRequest {
        descriptor: explicit_descriptor("ymap", 0, "engine.assets.world_map"),
    });

    let result = state.probe(AssetFileTypeProbeRequest {
        logical_path: "Maps/Forest.YMAP@main".to_owned(),
    });
    assert_eq!(result.extension, "ymap");
    assert_eq!(
        result.descriptor.unwrap().semantic_gateway,
        "engine.assets.world_map"
    );
}
