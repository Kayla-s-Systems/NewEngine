use super::*;

#[test]
fn text_gateway_is_engine_facing() {
    assert_eq!(ENGINE_TEXT_SERVICE_ID, "engine.ui.text");
    assert_eq!(
        TEXT_BACKEND_SERVICE_SPEC.engine_gateway_id,
        ENGINE_TEXT_SERVICE_ID
    );
    assert!(TextServiceInfo::default()
        .features
        .iter()
        .any(|feature| feature == "localization-v1"));
}

#[test]
fn service_contract_exposes_reference_architecture_domains() {
    for method in [
        TEXT_SERVICE_METHOD_LOCALIZE_V1,
        TEXT_SERVICE_METHOD_CATALOG_MANIFEST_V1,
        TEXT_SERVICE_METHOD_FORMAT_V1,
        TEXT_SERVICE_METHOD_MESSAGE_ENQUEUE_V1,
        TEXT_SERVICE_METHOD_MESSAGE_HISTORY_V1,
        TEXT_SERVICE_METHOD_PAGE_TEXT_V1,
        TEXT_SERVICE_METHOD_CONVERT_V1,
        TEXT_SERVICE_METHOD_SHAPE_RUN_V1,
        TEXT_SERVICE_METHOD_LAYOUT_PARAGRAPH_V1,
        TEXT_SERVICE_METHOD_ATLAS_PLAN_V1,
    ] {
        assert!(text_service_methods().contains(&method), "missing {method}");
    }
}

#[test]
fn existing_defaults_remain_compatible() {
    assert_eq!(TextShapeRunRequest::default().size_px, 16.0);
    assert_eq!(TextLayoutParagraphRequest::default().line_height_px, 20.0);
    assert_eq!(
        TextAtlasPlanRequest::default().max_page_size_px,
        [1024, 1024]
    );
    assert_eq!(
        TextShapeRunResponse::default().caret_positions_px,
        vec![0.0]
    );
}

#[test]
fn label_hash_is_ascii_case_insensitive() {
    assert_eq!(
        stable_text_key_hash("MISSION_START"),
        stable_text_key_hash("mission_start")
    );
    assert_ne!(
        stable_text_key_hash("mission_start"),
        stable_text_key_hash("mission_end")
    );
}

#[test]
fn token_component_scan_recognizes_original_and_indexed_forms() {
    assert_eq!(
        expected_format_components("Cash ~1~ / ~1_2~; player ~a~ versus ~a_0~"),
        (2, 2)
    );
    assert_eq!(
        filter_control_tokens("Press ~PAD_A~ to ~COL_RED~continue~COL_WHITE~"),
        "Press  to continue"
    );
}

#[test]
fn paged_text_defaults_to_four_explicit_paragraph_pages() {
    let response = paginate_text(&TextPageTextRequest {
        text: "one\n\ntwo\n\nthree\n\nfour\n\nfive".to_owned(),
        ..TextPageTextRequest::default()
    });
    assert_eq!(response.pages, ["one", "two", "three", "four"]);
    assert!(response.truncated);
}

#[test]
fn conversion_helpers_are_deterministic() {
    assert_eq!(format_human_integer(1_234_567, ','), "1,234,567");
    assert_eq!(format_human_integer(-42, ','), "-42");
    assert_eq!(format_milliseconds_short(61_400, true), "01:01");
    assert_eq!(format_milliseconds_long(3_661_042), "01:01:01.042");
}

#[test]
fn message_contract_round_trips_through_json() {
    let request = TextMessageEnqueueRequest {
        message: TextMessageDescriptor {
            channel: TextMessageChannel::Subtitle,
            text: TextMessageText::Localized(TextLookupKey::from_label("MISSION_START")),
            duration_ms: 4_000,
            add_to_history: true,
            ..TextMessageDescriptor::default()
        },
    };
    let json = serde_json::to_string(&request).expect("serialize");
    let decoded: TextMessageEnqueueRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.message.channel, TextMessageChannel::Subtitle);
    assert_eq!(decoded.message.duration_ms, 4_000);
}
