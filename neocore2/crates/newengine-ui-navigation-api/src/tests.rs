use super::*;

fn document_from_json(source: &str) -> UiNodeNavigationDocument {
    UiNodeNavigationDocument::from_json_str(source).expect("navigation document must parse")
}

#[test]
fn parse_minimal_document() {
    let document = document_from_json(
        r#"{
          "id":"engine.ui.primary",
          "surface_id":"engine.ui.primary",
          "root_page":"root",
          "pages":[{"id":"root","items":[{"id":"resume","label":"Resume"}]}]
        }"#,
    );
    document.validate().unwrap();
    assert_eq!(document.root().unwrap().items[0].label, "Resume");
}

#[test]
fn canonicalization_normalizes_nested_contracts() {
    let document = document_from_json(
        r#"{
          "id":" engine.ui.primary ",
          "surface_id":" engine.ui.primary ",
          "root_page":" root ",
          "footer_lines":["  Ready  ", "   "],
          "pages":[{
            "id":" root ",
            "items":[{
              "id":" play ",
              "label":" Play ",
              "value":"  ",
              "action":{
                "id":" open ",
                "source":" menu ",
                "target":" game ",
                "event":" accept ",
                "audio":"  ",
                "transition":{"kind":"open_page","page":" options "},
                "feedback":{"title":" Ready ","detail":" Go ","ttl_sec":100.0}
              }
            }]
          },{
            "id":"options",
            "parent_page":"root",
            "items":[{"id":"back","label":"Back"}]
          }]
        }"#,
    );

    assert_eq!(document.id, "engine.ui.primary");
    assert_eq!(document.footer_lines, ["Ready"]);
    let action = document.root().unwrap().items[0].action.as_ref().unwrap();
    assert_eq!(action.audio, None);
    assert_eq!(
        action.transition.as_ref().unwrap().page.as_deref(),
        Some("options")
    );
    assert_eq!(action.feedback.as_ref().unwrap().ttl_sec, 30.0);
    document.validate().unwrap();
}

#[test]
fn runtime_opens_page_and_dispatches_route() {
    let document = document_from_json(
        r#"{
          "id":"engine.ui.primary",
          "surface_id":"engine.ui.primary",
          "root_page":"root",
          "pages":[{
            "id":"root",
            "items":[{
              "id":"options",
              "label":"Options",
              "action":{
                "id":"open_options",
                "source":"root",
                "target":"options",
                "event":"accept",
                "transition":{"kind":"open_page","page":"options"}
              }
            }]
          },{
            "id":"options",
            "parent_page":"root",
            "items":[{"id":"volume","label":"Volume"}]
          }]
        }"#,
    );
    let mut runtime = UiNodeNavigationRuntime::new(document).unwrap();

    let output = runtime.handle_input(UiNodeNavigationInput {
        accept: true,
        ..UiNodeNavigationInput::default()
    });

    assert_eq!(runtime.current_page_id(), "options");
    assert_eq!(output.route_dispatches.len(), 1);
    assert_eq!(
        output.transition.unwrap().kind,
        UiNodeTransitionKind::OpenPage
    );
}

#[test]
fn runtime_wraps_vertical_selection() {
    let document = document_from_json(
        r#"{
          "id":"engine.ui.primary",
          "surface_id":"engine.ui.primary",
          "root_page":"root",
          "pages":[{
            "id":"root",
            "items":[
              {"id":"first","label":"First"},
              {"id":"second","label":"Second"}
            ]
          }]
        }"#,
    );
    let mut runtime = UiNodeNavigationRuntime::new(document).unwrap();

    runtime.handle_input(UiNodeNavigationInput {
        nav_y: -1,
        ..UiNodeNavigationInput::default()
    });

    assert_eq!(runtime.selected_index(), 1);
}
