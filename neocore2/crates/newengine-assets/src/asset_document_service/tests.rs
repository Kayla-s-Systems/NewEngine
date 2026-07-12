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
