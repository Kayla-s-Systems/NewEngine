use super::XmlSnippet;

pub(super) static MATERIAL_ROOT_SNIPPETS: &[XmlSnippet] = &[XmlSnippet {
    label: "Nemat Library",
    insert: r#"<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1" representation="xml" body_format="newengine.xml.metadata.v1">
  <Material name="material_name" shader="pbr.default">
    <Surface blend="opaque" two_sided="false" />
    <Textures>
    </Textures>
    <Params>
    </Params>
  </Material>
</NematMaterialLibrary>
"#,
    detail: "Root .nemat material library",
}];

pub(super) static MATERIAL_CHILD_SNIPPETS: &[XmlSnippet] = &[
    XmlSnippet {
        label: "Material",
        insert: r#"
  <Material name="material_name" shader="pbr.default">
    <Surface blend="opaque" two_sided="false" />
    <Textures>
    </Textures>
    <Params>
    </Params>
  </Material>"#,
        detail: "Addressable .nemat@entry material",
    },
    XmlSnippet {
        label: "Texture",
        insert: r#"
      <Texture slot="base_color" ref="textures/file.ytd@entry" />"#,
        detail: "Material texture slot -> .ytd@entry",
    },
    XmlSnippet {
        label: "Param",
        insert: r#"
      <Param name="roughness" type="float" value="0.8" />"#,
        detail: "Typed material parameter",
    },
];
