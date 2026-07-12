use super::XmlSnippet;

pub(super) static DEFINITION_ROOT_SNIPPETS: &[XmlSnippet] = &[XmlSnippet {
    label: "Ytyp Properties",
    insert: r#"<YtypProperties schema="newengine.ytyp.properties.v1" representation="xml" body_format="newengine.xml.properties.v1" name="asset_name" kind="game_ready_metadata" entry_kind="archetype_definition">
  <Dependencies>
    <Dependency role="drawable_dictionary" ref="models/example.ydd@example" required="true" />
  </Dependencies>
  <Metadata>
    <Namespace name="render">
      <Value key="mesh.role" value="world_opaque" />
    </Namespace>
  </Metadata>
</YtypProperties>
"#,
    detail: "Single-asset .ytyp properties document",
}];

pub(super) static DEFINITION_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet {
        label: "Entry",
        insert: r#"
  <Entry name="entry_name" kind="game_ready_metadata" entry_kind="archetype_definition">
    <Dependencies>
    </Dependencies>
  </Entry>"#,
        detail: "Addressable Definition Entry",
    },
    XmlSnippet {
        label: "Dependency",
        insert: r#"
      <Dependency domain="engine.assets.models" reference="path/file.ydd@entry" role="resource" required="true" />"#,
        detail: "Typed dependency edge",
    },
    XmlSnippet {
        label: "Namespace",
        insert: r#"
    <Metadata>
      <Namespace name="newengine.game_ready">
      </Namespace>
    </Metadata>"#,
        detail: "Domain metadata namespace",
    },
];
