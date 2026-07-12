use super::XmlSnippet;

pub(super) static MAP_ROOT_SNIPPETS: &[XmlSnippet] = &[XmlSnippet {
    label: "Ymap Map",
    insert: r#"<YmapMapDefinition schema="newengine.map.definition.v1" representation="xml" body_format="newengine.xml.metadata.v1">
  <map name="map_name">
    <definition_refs>
    </definition_refs>
    <placements>
    </placements>
  </map>
</YmapMapDefinition>
"#,
    detail: "Root authored map definition",
}];

pub(super) static MAP_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet {
        label: "DefinitionRef",
        insert: r#"
      <DefinitionRef value="world/file.ytyp@entry" />"#,
        detail: "Referenced Definition Entry",
    },
    XmlSnippet {
        label: "Placement",
        insert: r#"
      <Placement definition_ref="world/file.ytyp@entry" position="0,0,0" rotation_ypr="0,0,0" scale="1,1,1" apply_mode="metadata_only" />"#,
        detail: "Map placement/apply command",
    },
];
