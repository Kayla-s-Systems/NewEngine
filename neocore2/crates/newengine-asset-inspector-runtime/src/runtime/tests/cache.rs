use super::super::presentation::document_exposes_entries;
use super::super::*;

#[test]
fn document_cache_is_bounded_and_promotes_hits() {
    let mut cache = DocumentCache::default();
    for index in 0..(DOCUMENT_CACHE_CAPACITY + 2) {
        cache.insert(&AssetDocument {
            asset_ref: format!("asset-{index}.json"),
            ..Default::default()
        });
    }
    assert_eq!(cache.entries.len(), DOCUMENT_CACHE_CAPACITY);
    assert!(cache.get("asset-0.json").is_none());
    assert!(cache.get("asset-9.json").is_some());
    assert_eq!(cache.entries.front().unwrap().0, "asset-9.json");
}

#[test]
fn list_file_descriptor_exposes_entries_without_decoding_manifest() {
    let document = AssetDocument {
        descriptor: Some(newengine_assets_api::AssetFileTypeDescriptor {
            codec_type: "listFile".to_owned(),
            selector_syntax: Some("@entry".to_owned()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(document_exposes_entries(&document));
}
