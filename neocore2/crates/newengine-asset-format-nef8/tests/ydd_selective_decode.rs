use newengine_asset_format_nef8::ydd_binary::{
    decode_ydd_binary_entries, decode_ydd_binary_entry, encode_ydd_binary_body, YddBinaryDocument,
    YddBinaryEntry, YddBinaryMesh, YddBinaryVertex,
};

fn entry(name: &str, x_offset: f32) -> YddBinaryEntry {
    YddBinaryEntry {
        name: name.to_owned(),
        source_path: format!("source/{name}.gltf"),
        properties_ref: Some(format!("definitions/{name}.ytyp")),
        bounds_min: [x_offset, 0.0, 0.0],
        bounds_max: [x_offset + 1.0, 1.0, 0.0],
        skin_source_to_model: None,
        meshes: vec![YddBinaryMesh {
            name: format!("{name}_mesh"),
            material_ref: Some(format!("materials/test.nemat@{name}")),
            bounds_min: [x_offset, 0.0, 0.0],
            bounds_max: [x_offset + 1.0, 1.0, 0.0],
            vertices: vec![
                YddBinaryVertex {
                    position: [x_offset, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv0: [0.0, 0.0],
                },
                YddBinaryVertex {
                    position: [x_offset + 1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv0: [1.0, 0.0],
                },
                YddBinaryVertex {
                    position: [x_offset, 1.0, 0.0],
                    normal: [0.0, 0.0, 1.0],
                    uv0: [0.0, 1.0],
                },
            ],
            skin: None,
            indices: vec![0, 1, 2],
        }],
    }
}

#[test]
fn selected_entry_decode_does_not_materialize_unrequested_entries() {
    let encoded = encode_ydd_binary_body(&YddBinaryDocument {
        entries: vec![entry("stadium_main", 0.0), entry("stadium_stands", 10.0)],
    })
    .expect("encode YDD dictionary");

    let selected = decode_ydd_binary_entries(&encoded, &["stadium_stands".to_owned()])
        .expect("selectively decode one entry");
    assert_eq!(selected.entries.len(), 1);
    assert_eq!(selected.entries[0].name, "stadium_stands");
    assert_eq!(selected.entries[0].meshes[0].vertices[0].position[0], 10.0);

    let single =
        decode_ydd_binary_entry(&encoded, "stadium_main").expect("decode one selected entry");
    assert_eq!(single.name, "stadium_main");
    assert_eq!(single.meshes[0].vertices[0].position[0], 0.0);
}

#[test]
fn selected_entry_decode_rejects_unknown_entry() {
    let encoded = encode_ydd_binary_body(&YddBinaryDocument {
        entries: vec![entry("stadium_main", 0.0)],
    })
    .expect("encode YDD dictionary");
    let error = decode_ydd_binary_entry(&encoded, "missing")
        .expect_err("unknown selector must fail closed");
    assert!(error.contains("selector 'missing' was not found"));
}
