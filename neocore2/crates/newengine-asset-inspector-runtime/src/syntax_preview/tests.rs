use super::*;

fn layer_text(row: &SyntaxPreviewRow, class: SyntaxClass) -> &str {
    &row.layers[class as usize]
}

#[test]
fn json_profile_distinguishes_keys_values_numbers_and_symbols() {
    let lines = vec![r#"{"age": 25, "name": "Kayla", "active": true}"#.to_owned()];
    let page = highlight_preview_page(&lines, "json", 0);
    let row = &page.rows[0];
    assert!(layer_text(row, SyntaxClass::Attribute).contains("\"age\""));
    assert!(layer_text(row, SyntaxClass::String).contains("\"Kayla\""));
    assert!(layer_text(row, SyntaxClass::Number).contains("25"));
    assert!(layer_text(row, SyntaxClass::Reserved).contains("true"));
    assert!(layer_text(row, SyntaxClass::Symbol).contains(':'));
}

#[test]
fn xml_profile_preserves_multiline_comment_state_and_attributes() {
    let lines = vec![
        "<!-- comment".to_owned(),
        "continued --> <node id=\"42\">value</node>".to_owned(),
    ];
    let page = highlight_preview_page(&lines, "xml", 0);
    assert!(layer_text(&page.rows[0], SyntaxClass::Comment).contains("<!-- comment"));
    assert!(layer_text(&page.rows[1], SyntaxClass::Comment).contains("continued -->"));
    assert!(layer_text(&page.rows[1], SyntaxClass::Reserved).contains("node"));
    assert!(layer_text(&page.rows[1], SyntaxClass::Attribute).contains("id"));
    assert!(layer_text(&page.rows[1], SyntaxClass::String).contains("\"42\""));
}

#[test]
fn ini_profile_highlights_comment_section_key_and_value() {
    let lines = vec![
        "; Comment".to_owned(),
        "[Section]".to_owned(),
        "Name=Value".to_owned(),
    ];
    let page = highlight_preview_page(&lines, "ini", 0);
    assert!(layer_text(&page.rows[0], SyntaxClass::Comment).contains("; Comment"));
    assert!(layer_text(&page.rows[1], SyntaxClass::Reserved).contains("Section"));
    assert!(layer_text(&page.rows[2], SyntaxClass::Attribute).contains("Name"));
    assert!(layer_text(&page.rows[2], SyntaxClass::String).contains("Value"));
}

#[test]
fn text_profile_highlights_supported_uri_prefixes() {
    let lines = vec!["Open https://example.com or mailto:kayla@example.com".to_owned()];
    let page = highlight_preview_page(&lines, "text", 0);
    let links = layer_text(&page.rows[0], SyntaxClass::Link);
    assert!(links.contains("https://example.com"));
    assert!(links.contains("mailto:kayla@example.com"));
}

#[test]
fn editor_projection_covers_sixteen_rows_and_wider_source() {
    let lines = (0..20)
        .map(|index| format!("let value_{index} = {index}; // editor row"))
        .collect::<Vec<_>>();
    let page = highlight_editor_page(&lines, "rust", 0);
    assert_eq!(page.rows.len(), SYNTAX_EDITOR_ROWS);
    assert!(page.rows[0].layers[SyntaxClass::Reserved as usize].contains("let"));
    assert!(page.rows[0]
        .layers
        .iter()
        .all(|layer| { layer.chars().count() <= SYNTAX_EDITOR_COLUMNS }));
}

#[test]
fn all_masks_keep_equal_character_width() {
    let lines = vec!["\t{\"ключ\": 42}".to_owned()];
    let page = highlight_preview_page(&lines, "json", 0);
    let lengths = page.rows[0]
        .layers
        .iter()
        .map(|layer| layer.chars().count())
        .collect::<Vec<_>>();
    assert!(lengths.windows(2).all(|pair| pair[0] == pair[1]));
}
