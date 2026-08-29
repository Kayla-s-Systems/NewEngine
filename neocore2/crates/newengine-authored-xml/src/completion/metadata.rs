use super::XmlSnippet;

pub(super) static METADATA_ROOT_SNIPPETS: &[XmlSnippet] = &[XmlSnippet {
    label: "Ymt Metadata",
    insert: r#"<YmtMetadata schema="newengine.ymt.metadata.v1" representation="xml" body_format="newengine.xml.metadata.v1">
  <Entry name="metadata_entry">
  </Entry>
</YmtMetadata>
"#,
    detail: "Root .ymt metadata container",
}];
