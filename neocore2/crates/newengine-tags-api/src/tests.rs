use super::*;

#[test]
fn tag_id_preserves_value() {
    let tag = TagId::new("gameplay.interactable");
    assert_eq!(tag.as_str(), "gameplay.interactable");
}

#[test]
fn service_info_exposes_contract_methods() {
    let info = TagsServiceInfoV1::default();
    assert_eq!(info.protocol, TAGS_RUNTIME_CONTRACT);
    assert!(info
        .methods
        .iter()
        .any(|method| method == tags_method::VALIDATE_TAG_SET_JSON_V1));
}

#[test]
fn validate_set_response_roundtrips_json() {
    let response = TagsValidateSetResponseV1 {
        accepted: true,
        normalized_tags: vec![TagId::new("state.active")],
        ..TagsValidateSetResponseV1::default()
    };
    let encoded = serde_json::to_string(&response).unwrap();
    let decoded: TagsValidateSetResponseV1 = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, response);
}
