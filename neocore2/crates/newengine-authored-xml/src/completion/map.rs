use super::XmlSnippet;

pub(super) static MAP_ROOT_SNIPPETS: &[XmlSnippet] = &[XmlSnippet {
    label: "Ymap Discrete Map",
    insert: r#"<YmapMapDefinition schema="newengine.map.definition.v2" representation="xml" body_format="newengine.xml.metadata.v1">
  <map id="map_name" cell_size="64" origin="0,0,0">
    <cells>
      <Cell x="0" z="0">
        <placements>
        </placements>
      </Cell>
    </cells>
  </map>
</YmapMapDefinition>
"#,
    detail: "Discrete map: root index plus independently addressable cells",
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
        insert: r#"<Placement id="instance_name" definition_ref="world/file.ytyp@entry" position="0,0,0" rotation_ypr="0,0,0" scale="1,1,1" apply_mode="instantiate" />"#,
        detail: "Placement inside one discrete cell",
    },
    XmlSnippet {
        label: "Cell",
        insert: r#"<Cell x="0" z="0">
  <placements></placements>
</Cell>"#,
        detail: "Independently replaceable/streamable map cell",
    },
];
