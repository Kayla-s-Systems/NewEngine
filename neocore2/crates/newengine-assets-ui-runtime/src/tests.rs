use super::*;

use crate::compile_request::validate_requested_entry;

#[test]
fn parses_navigation_document_from_xmlcentral() {
    let xml = r#"<NeUiDictionary><UiNodeNavigationDocument id="engine.ui.primary" surface_id="engine.ui.primary" root_page="root" title="UI"><Page id="root"><Item id="resume" label="Resume"><Action id="resume" source="s" target="UiNodeNavigationRuntime" event="ui.close" transition="close" /></Item></Page></UiNodeNavigationDocument></NeUiDictionary>"#;

    let doc = parse_navigation_document(xml).unwrap().unwrap();

    assert_eq!(doc.id, "engine.ui.primary");

    assert_eq!(doc.pages[0].items[0].label, "Resume");
}

#[test]
fn derives_navigation_document_from_surface_layout_buttons() {
    let xml = r#"

<NeUiDictionary document_kind="surface">

  <Surface name="engine.main_menu" kind="main_menu" modal="true" z_order="700" root="layout.main" />

  <Layout name="layout.main" surface="engine.main_menu">

    <Panel id="main.root" class="menu-shell">

      <Text id="main.title" class="title" value="NewEngine" />

      <Button id="main.start" class="button button-primary"><Text value="Start" /><Event trigger="click" action="game.start" /></Button>

      <Button id="main.settings" class="button button-secondary"><Text value="Settings" /><Event trigger="click" action="engine.settings.open" /></Button>

    </Panel>

  </Layout>

  <ActionMap name="actions">

    <Action id="engine.settings.open" target="engine.ui.navigation" command="menu.open_page"><Payload page="settings" /></Action>

  </ActionMap>

</NeUiDictionary>

"#;

    let surface = SurfaceInfo {
        name: "engine.main_menu".to_owned(),

        kind: "main_menu".to_owned(),

        root: "layout.main".to_owned(),

        theme: None,

        modal: true,

        z_order: 700,
    };

    let doc = derive_navigation_document_from_surface_layout(xml, &surface)
        .unwrap()
        .unwrap();

    assert_eq!(doc.surface_id, "engine.main_menu");

    assert_eq!(doc.pages[0].items[0].label, "Start");

    assert_eq!(doc.pages[0].items[1].label, "Settings");
}

#[test]
fn compiles_neui_surface_layout_into_root_node_request() {
    let xml = r#"

<NeUiDictionary document_kind="surface">

  <Surface name="engine.main_menu" kind="main_menu" modal="true" z_order="700" root="layout.main" theme="assets/ui/themes/northstar_editor.neui@editor_light" />

  <Layout name="layout.main" surface="engine.main_menu">

    <Panel id="main.root" class="menu-shell">

      <Text id="main.title" class="title" value="NewEngine" />

      <Button id="main.start" class="button button-primary"><Text value="Start" /><Event trigger="click" action="game.start" /></Button>

      <Select id="graphics.quality"><Option id="graphics.high" label="High" value="high" /></Select>

    </Panel>

  </Layout>

</NeUiDictionary>

"#;

    let surface = SurfaceInfo {
        name: "engine.main_menu".to_owned(),

        kind: "main_menu".to_owned(),

        root: "layout.main".to_owned(),

        theme: Some("assets/ui/themes/northstar_editor.neui@editor_light".to_owned()),

        modal: true,

        z_order: 700,
    };

    let dialect = NeUiDialect::builtin();
    let root = compile_surface_root(
        xml,
        &surface,
        "assets/ui/frontend/main_menu.neui@surface",
        surface.theme.as_deref(),
        &dialect,
    )
    .unwrap();

    assert_eq!(root.kind, UiRuntimeNodeKind::Surface);

    assert_eq!(root.children.len(), 1);

    let panel = &root.children[0];

    assert_eq!(panel.id, "main.root");

    let button = panel
        .children
        .iter()
        .find(|node| node.id == "main.start")
        .unwrap();

    assert_eq!(button.action_id.as_deref(), Some("game.start"));

    assert!(button.interactive);

    let select = panel
        .children
        .iter()
        .find(|node| node.id == "graphics.quality")
        .unwrap();

    assert!(select.interactive);

    assert_eq!(
        select.children[0].action_id.as_deref(),
        Some("ui.select.high")
    );
}

fn find_node<'a>(node: &'a UiNodeRequest, id: &str) -> Option<&'a UiNodeRequest> {
    if node.id == id {
        return Some(node);
    }

    node.children.iter().find_map(|child| find_node(child, id))
}

fn aurelia_asset_preview_fixture_xml() -> String {
    #[cfg(not(miri))]
    {
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

        let workspace_root = manifest_dir
            .ancestors()
            .nth(2)
            .expect("crate manifest must live under neocore2/crates");

        let candidate =
            workspace_root.join("assets/ui/src/devtools/aurelia_asset_preview_stand.neui.xml");
        if candidate.exists() {
            return std::fs::read_to_string(&candidate)
                .unwrap_or_else(|err| panic!("failed to read {}: {}", candidate.display(), err));
        }
    }

    aurelia_asset_preview_embedded_fixture_xml()
}

fn aurelia_asset_preview_embedded_fixture_xml() -> String {
    r#"

<NeUiDictionary document_kind="surface">

  <ThemeRef ref="assets/ui/themes/northstar_editor.neui@editor_light" />

  <TextureRef ref="assets/textures/ui/loading/loading_ui.ytd@newengine_logo" />

  <TextureRef ref="assets/textures/ui/icons/builtin_icons.ytd@folder" />

  <FontRef ref="assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold" />

  <Surface name="engine.ui.aurelia_asset_preview_stand" kind="devtools" root="preview.root" theme="assets/ui/themes/northstar_editor.neui@editor_light" />

  <Layout name="preview.root" surface="engine.ui.aurelia_asset_preview_stand">

    <Panel id="preview.root">

      <Text id="preview.title" value="AureliaUI asset preview stand" font="assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold" />

      <Image id="preview.logo" texture="assets/textures/ui/loading/loading_ui.ytd@newengine_logo" />

      <Icon id="preview.folder" icon="assets/textures/ui/icons/builtin_icons.ytd@folder" />

      <Text id="preview.caption" value="Preview caption" font="assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold" />

    </Panel>

  </Layout>

  <BindingGraph id="preview.bindings">

    <StateSource id="preview" source="engine.assets" contract="asset.preview" update="frame" />

    <Bind element="preview.logo" property="texture" source_id="preview" source="texture" />

    <Bind element="preview.caption" property="text" source_id="preview" source="caption" />

  </BindingGraph>

  <Action id="ui.preview.refresh" element="preview.root" trigger="click" target="engine.ui" command="preview.refresh" />

</NeUiDictionary>

"#

        .to_owned()
}

#[test]
fn compiles_aurelia_asset_preview_stand_with_font_texture_and_binding_refs() {
    let xml = aurelia_asset_preview_fixture_xml();

    let document_ref = "assets/ui/devtools/aurelia_asset_preview_stand.neui@surface";

    validate_requested_entry(&xml, "surface").unwrap();

    let dependencies = extract_dependencies(&xml);

    assert!(dependencies
        .iter()
        .any(|dep| dep == "assets/textures/ui/loading/loading_ui.ytd@newengine_logo"));

    assert!(dependencies
        .iter()
        .any(|dep| dep == "assets/textures/ui/icons/builtin_icons.ytd@folder"));

    assert!(dependencies
        .iter()
        .any(|dep| dep == "assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold"));

    let surface = parse_surface(&xml).expect("preview stand must declare a surface");

    assert_eq!(surface.name, "engine.ui.aurelia_asset_preview_stand");

    assert_eq!(
        surface.theme.as_deref(),
        Some("assets/ui/themes/northstar_editor.neui@editor_light")
    );

    let dialect = NeUiDialect::builtin();
    let root = compile_surface_root(
        &xml,
        &surface,
        document_ref,
        surface.theme.as_deref(),
        &dialect,
    )
    .unwrap();

    assert_eq!(root.kind, UiRuntimeNodeKind::Surface);

    assert_eq!(
        root.props.get("theme_ref").and_then(|value| value.as_str()),
        Some("assets/ui/themes/northstar_editor.neui@editor_light")
    );

    let title = find_node(&root, "preview.title").expect("title text node must compile");

    assert_eq!(title.kind, UiRuntimeNodeKind::Text);

    assert_eq!(
        title.font_token.as_deref(),
        Some("assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold")
    );

    assert_eq!(title.text, "AureliaUI asset preview stand");

    let logo = find_node(&root, "preview.logo").expect("texture image node must compile");

    assert_eq!(logo.kind, UiRuntimeNodeKind::ExternalTexture);

    assert_eq!(
        logo.icon.as_deref(),
        Some("assets/textures/ui/loading/loading_ui.ytd@newengine_logo")
    );

    let folder = find_node(&root, "preview.folder").expect("icon node must compile");

    assert_eq!(folder.kind, UiRuntimeNodeKind::ExternalTexture);

    assert_eq!(
        folder.icon.as_deref(),
        Some("assets/textures/ui/icons/builtin_icons.ytd@folder")
    );

    let caption = find_node(&root, "preview.caption").expect("caption text node must compile");

    assert_eq!(
        caption.font_token.as_deref(),
        Some("assets/ui/fonts/editor.yfd@tt_lakes_neue_trial_bold")
    );

    let binding_plan = parse_binding_plan(&xml, document_ref, &surface.name);

    assert_eq!(binding_plan.state_sources.len(), 1);

    assert!(binding_plan
        .bindings
        .iter()
        .any(|binding| binding.element_id == "preview.logo" && binding.property == "texture"));

    assert!(binding_plan
        .bindings
        .iter()
        .any(|binding| binding.element_id == "preview.caption" && binding.property == "text"));

    assert!(binding_plan
        .actions
        .iter()
        .any(|action| action.action_id == "ui.preview.refresh"
            && action.target_gateway == "engine.ui"));
}

#[test]
fn visible_binding_fallback_initializes_retained_node_before_first_patch() {
    let xml = r#"
<NeUiDictionary document_kind="surface">
  <Surface name="game.hud" kind="game_hud" root="layout.main" />
  <Layout name="layout.main" surface="game.hud" title="">
    <Panel id="character.window">
      <Bind property="visible" source="character.open" fallback="false" />
      <Text id="character.title" text="Player Setup" />
    </Panel>
  </Layout>
</NeUiDictionary>
"#;
    let surface = parse_surface(xml).expect("surface declaration");
    let root = compile_surface_root(
        xml,
        &surface,
        "ui/shared/runtime/hud.neui@surface",
        None,
        &NeUiDialect::builtin(),
    )
    .expect("compile HUD");
    assert!(
        root.text.is_empty(),
        "surface root must not leak its id as visible text"
    );
    let window = find_node(&root, "character.window").expect("character window");
    assert!(
        !window.visible,
        "visible fallback=false must apply before the first state patch"
    );
    assert_eq!(
        window.children.len(),
        1,
        "hidden initial state must retain the full subtree"
    );
}

#[test]
fn compiled_absolute_image_preserves_authored_position_and_extent() {
    let xml = r#"
<NeUiDictionary document_kind="surface">
  <Surface name="engine.test.absolute_image" kind="main_menu" root="layout.main" />
  <Layout name="layout.main" surface="engine.test.absolute_image">
    <Image id="menu.logo"
           position="absolute"
           x_px="64" y_px="48" w_px="512" h_px="144"
           texture="textures/ui/menu/main_menu_logo.ytd@logo" />
  </Layout>
</NeUiDictionary>
"#;
    let surface = parse_surface(xml).expect("surface declaration");
    let root = compile_surface_root(
        xml,
        &surface,
        "ui/engine/test.neui@surface",
        None,
        &NeUiDialect::builtin(),
    )
    .expect("compile absolute image");
    let image = find_node(&root, "menu.logo").expect("compiled image node");
    assert_eq!(image.kind, UiRuntimeNodeKind::ExternalTexture);
    assert_eq!(
        image
            .props
            .get("position")
            .and_then(serde_json::Value::as_str),
        Some("absolute")
    );
    assert_eq!(
        image.props.get("x_px").and_then(serde_json::Value::as_f64),
        Some(64.0)
    );
    assert_eq!(
        image.props.get("y_px").and_then(serde_json::Value::as_f64),
        Some(48.0)
    );
    assert_eq!(
        image.props.get("w_px").and_then(serde_json::Value::as_f64),
        Some(512.0)
    );
    assert_eq!(
        image.props.get("h_px").and_then(serde_json::Value::as_f64),
        Some(144.0)
    );
}
