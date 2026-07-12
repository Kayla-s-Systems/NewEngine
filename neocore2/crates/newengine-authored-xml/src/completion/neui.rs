use super::XmlSnippet;

pub(super) static NEUI_ROOT_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet {
        label: "NeUi Surface Dictionary",
        insert: r##"<NeUiDictionary schema="newengine.neui.dictionary.v1" representation="xmlcentral" owner_scope="engine" document_kind="surface">


  <Surface name="engine.ui.loading" root="layout.main" theme="assets/ui/themes/north_star_dark.neui@theme" bindings="bindings">


    <Dependencies>


    </Dependencies>


  </Surface>





  <Layout name="layout.main" surface="engine.ui.loading">


    <Panel id="root" class="surface-shell" />


  </Layout>





  <BindingGraph name="bindings">


  </BindingGraph>


</NeUiDictionary>


"##,
        detail: "Root .neui surface dictionary",
    },
    XmlSnippet {
        label: "NeUi Registry",
        insert: r##"<NeUiRegistry schema="newengine.neui.registry.v1">


  <Surfaces>


    <SurfaceRef id="engine.ui.loading" ref="assets/ui/engine/loading.neui@surface" />


  </Surfaces>


  <Themes>


    <ThemeRef id="north_star.dark" ref="assets/ui/themes/north_star_dark.neui@theme" />


  </Themes>


  <ComponentPacks>


  </ComponentPacks>


</NeUiRegistry>


"##,
        detail: "Registry of UI refs only; no inline layouts",
    },
    XmlSnippet {
        label: "NeUi Theme Library",
        insert: r##"<NeUiThemeLibrary schema="newengine.neui.theme.v1" representation="xmlcentral" owner_scope="shared" document_kind="theme">


  <Theme name="north_star.dark">


    <Token name="color.bg" value="#0B0D10" />


    <Token name="color.accent" value="#FF7A18" />


  </Theme>


</NeUiThemeLibrary>


"##,
        detail: "Theme tokens split from surfaces",
    },
];

pub(super) static NEUI_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet {
        label: "Surface",
        insert: r##"


  <Surface name="engine.ui.loading" root="layout.main" theme="assets/ui/themes/north_star_dark.neui@theme" bindings="bindings">


    <Dependencies>


      <ComponentRef ref="assets/ui/components/cards.neui@card.status" />


      <TextureRef ref="assets/textures/ui/icons/builtin_icons.ytd@app_logo" />


    </Dependencies>


  </Surface>"##,
        detail: "Addressable UI surface entry",
    },
    XmlSnippet {
        label: "Layout",
        insert: r##"


  <Layout name="layout.main" surface="engine.ui.loading">


    <Panel id="root" class="surface-shell" />


  </Layout>"##,
        detail: "UI layout tree",
    },
    XmlSnippet {
        label: "BindingGraph",
        insert: r##"


  <BindingGraph name="bindings">


    <StateSource id="loading" source="engine.ui.loading.status" contract="LoadingStatusSnapshot" update="event" />


    <Bind element="loading.progress" property="value" source="loading.progress" />


  </BindingGraph>"##,
        detail: "Declarative state binding plan",
    },
    XmlSnippet {
        label: "ActionMap",
        insert: r##"


  <ActionMap name="actions">


    <Action id="game.resume" target="engine.lifecycle" command="game.resume" />


  </ActionMap>"##,
        detail: "UI actions routed through engine gateway contracts",
    },
    XmlSnippet {
        label: "ComponentRef",
        insert: r##"


      <ComponentRef ref="assets/ui/components/buttons.neui@button.primary" />"##,
        detail: "Reference reusable component entry",
    },
];

pub const NEUI_ROOT_NAMES: &[&str] = &[
    "NeUiDictionary",
    "NeUiRegistry",
    "NeUiThemeLibrary",
    "NeUiComponentLibrary",
    "NeUiBindingLibrary",
];

#[inline]
pub fn is_neui_root_name(name: &str) -> bool {
    NEUI_ROOT_NAMES.contains(&name)
}
