use super::*;
use newengine_materials::{MaterialFlags, ShadingModel};

#[test]
fn runtime_texture_ref_rejects_raw_source_images() {
    assert!(
        strict_runtime_texture_ref("player/abigail/textures/hair_diff_000_a_uni.dds").is_none()
    );
    assert!(strict_runtime_texture_ref("textures/foo.png").is_none());
    assert!(strict_runtime_texture_ref("textures/foo.jpg").is_none());
}

#[test]
fn runtime_texture_ref_rejects_ytd_without_entry() {
    assert!(strict_runtime_texture_ref("textures/world.ytd").is_none());
}

#[test]
fn runtime_texture_ref_accepts_ytd_entry() {
    assert_eq!(
        strict_runtime_texture_ref("textures/world.ytd@brick_albedo").as_deref(),
        Some("textures/world.ytd@brick_albedo")
    );
}

#[test]
fn nemat_entry_selector_is_first_class() {
    let (path, selector) =
        split_nemat_selector("materials/world_garage.nemat@garage_door", None).unwrap();
    assert_eq!(path, "materials/world_garage.nemat");
    assert_eq!(selector, "garage_door");
}

#[test]
fn nemat_without_entry_is_rejected() {
    let err = split_nemat_selector("materials/world_garage.nemat", None).unwrap_err();
    assert!(err.contains("@entry"));
}

#[test]
fn preview_selector_prefers_opaque_base_color_material_over_glass() {
    let body = br#"<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1">
            <Material name="cpp_glasses" shader="pbr.default">
                <Surface blend="alpha" two_sided="True" />
                <Textures><Texture slot="normal" ref="textures/test.ytd@normal" /></Textures>
                <Params><Param name="base_color" type="color" value="0.03,0.03,0.03,0.32" /></Params>
            </Material>
            <Material name="material" shader="pbr.default">
                <Surface blend="opaque" two_sided="False" />
                <Textures><Texture slot="base_color" ref="textures/test.ytd@base" /></Textures>
                <Params><Param name="base_color" type="color" value="1,1,1,1" /></Params>
            </Material>
        </NematMaterialLibrary>"#;

    assert_eq!(preview_material_name_from_body(body).unwrap(), "material");
}

#[test]
fn preview_selector_falls_back_to_first_named_material_when_all_are_transparent() {
    let body = br#"<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1">
            <Material name="glass_a" shader="pbr.default"><Surface blend="alpha" /></Material>
            <Material name="glass_b" shader="pbr.default"><Surface blend="alpha" /></Material>
        </NematMaterialLibrary>"#;

    assert_eq!(preview_material_name_from_body(body).unwrap(), "glass_a");
}

#[test]
fn material_library_payload_selects_entry() {
    let payload = br#"<?xml version="1.0" encoding="UTF-8"?>
<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1">
  <Material name="garage_door" shader="pbr.default">
    <Surface blend="opaque" two_sided="false" />
    <Textures>
      <Texture slot="base_color" ref="textures/world/garage.ytd@garage_door_bc" />
    </Textures>
    <Params>
      <Param name="roughness" type="float" value="0.7" />
    </Params>
  </Material>
</NematMaterialLibrary>
"#;
    let material = decode_material_entry_payload(payload, "garage_door").unwrap();
    assert_eq!(material.name, "garage_door");
    assert!(material.textures.contains_key("base_color"));
}

#[test]
fn authored_uv_transform_populates_texture_bindings() {
    let payload = br#"<?xml version="1.0" encoding="UTF-8"?>
<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1">
  <Material name="terrain" shader="pbr.default">
    <Surface blend="opaque" two_sided="false" />
    <Textures>
      <Texture slot="base_color" ref="textures/world/terrain.ytd@grass" />
    </Textures>
    <Params>
      <Param name="uv_scale" type="float2" value="128,96" />
      <Param name="uv_offset" type="float2" value="0.25,-0.5" />
    </Params>
  </Material>
</NematMaterialLibrary>
"#;
    let material = decode_material_entry_payload(payload, "terrain").unwrap();
    let response = material_response_from_authored("materials/test.nemat", "terrain", material)
        .expect("authored material response");
    assert_eq!(response.textures.uv_scale, [128.0, 96.0]);
    assert_eq!(response.textures.uv_offset, [0.25, -0.5]);
    assert!(!response
        .descriptor
        .flags
        .contains(MaterialFlags::DOUBLE_SIDED));
}

#[test]
fn canonical_xmltype_schema_is_runtime_readable() {
    let body = br#"<NematMaterialLibrary schema="newengine.nemat.xmltype.v1" version="1"><Material name="p4" shader="pbr.default" /></NematMaterialLibrary>"#;
    let library = crate::decode_nemat_material_library_from_body(body)
        .expect("canonical NEMAT XMLtype schema");
    assert_eq!(library.materials.len(), 1);
    assert_eq!(library.materials[0].name, "p4");
}

#[test]
fn legacy_material_library_xml_schema_remains_runtime_readable() {
    let body = br#"<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1"><Material name="legacy" shader="pbr.default" /></NematMaterialLibrary>"#;
    let library = crate::decode_nemat_material_library_from_body(body)
        .expect("legacy NEMAT authored XML schema");
    assert_eq!(library.materials[0].name, "legacy");
}

#[test]
fn pbr_eye_shader_selects_eye_shading_model() {
    let payload =
        br#"<NematMaterialLibrary schema="newengine.nemat.material_library.v1" version="1">
  <Material name="eye" shader="pbr.eye" domain="surface">
    <Surface blend="opaque" two_sided="false" />
    <Textures>
      <Texture slot="base_color" ref="textures/characters/test.ytd@eye_base" />
      <Texture slot="roughness" ref="textures/characters/test.ytd@eye_roughness" />
    </Textures>
  </Material>
</NematMaterialLibrary>"#;
    let material = decode_material_entry_payload(payload, "eye").expect("eye authored material");
    let response = material_response_from_authored("materials/test.nemat", "eye", material)
        .expect("eye runtime material response");
    assert_eq!(response.descriptor.shading_model, ShadingModel::Eye);
}
