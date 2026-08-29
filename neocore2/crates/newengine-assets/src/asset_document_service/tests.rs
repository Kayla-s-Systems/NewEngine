use super::*;

#[test]
fn asset_reference_normalization_is_stable() {
    assert_eq!(
        normalize_asset_ref(r"./Textures\\World\\Road.YTD@Diffuse"),
        "Textures/World/Road.YTD@Diffuse"
    );
    assert_eq!(
        split_entry_ref("maps/world.ymap@main"),
        ("maps/world.ymap".to_owned(), Some("main".to_owned()))
    );
    assert_eq!(path_extension("maps/world.YMAP"), "ymap");
    assert_eq!(file_name("maps/world.ymap"), "world.ymap");
}

#[test]
fn read_only_document_actions_do_not_enable_writeback() {
    let document = AssetDocument {
        asset_ref: "maps/world.ymap@main".to_owned(),
        can_apply_patch: false,
        ..AssetDocument::default()
    };
    let actions = asset_document_actions(&document, Some("main"));
    assert!(!actions.is_empty());
    assert!(actions.iter().all(|action| !action.enabled));
}

#[test]
fn delete_entry_is_staged_and_rebuild_requires_dirty_state() {
    let clean = AssetDocument {
        asset_ref: "textures/world.ytd@albedo".to_owned(),
        provider_service: "engine.assets.textures".to_owned(),
        edit_contract: "asset.edit.ytd.v1".to_owned(),
        can_apply_patch: true,
        dirty: false,
        ..AssetDocument::default()
    };
    let clean_actions = asset_document_actions(&clean, Some("albedo"));
    let delete = clean_actions
        .iter()
        .find(|action| action.id == asset_document_action_id::DELETE)
        .expect("delete action");
    assert!(delete.enabled);
    assert_eq!(delete.method, asset_edit_method::STAGE_PATCH_JSON_V1);
    assert_eq!(
        delete
            .patch_template
            .as_ref()
            .and_then(|patch| patch.operations.first())
            .map(|operation| operation.op.as_str()),
        Some("delete")
    );
    let rebuild_clean = clean_actions
        .iter()
        .find(|action| action.id == asset_document_action_id::REBUILD)
        .expect("rebuild action");
    assert!(!rebuild_clean.enabled);

    let dirty = AssetDocument {
        dirty: true,
        ..clean
    };
    let dirty_actions = asset_document_actions(&dirty, Some("albedo"));
    let rebuild_dirty = dirty_actions
        .iter()
        .find(|action| action.id == asset_document_action_id::REBUILD)
        .expect("rebuild action");
    assert!(rebuild_dirty.enabled);
    assert_eq!(rebuild_dirty.method, asset_edit_method::REBUILD_JSON_V1);
    assert_eq!(
        rebuild_dirty
            .patch_template
            .as_ref()
            .map(|patch| patch.asset_ref.as_str()),
        Some("textures/world.ytd")
    );
}
