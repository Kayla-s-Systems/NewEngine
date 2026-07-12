use super::decode::{BODY_HEADER_LEN, ENTRY_RECORD_LEN, MESH_HEADER_LEN, VERTEX_STRIDE};
use super::*;

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_vec3(out: &mut Vec<u8>, value: [f32; 3]) {
    for component in value {
        push_f32(out, component);
    }
}

fn test_body() -> Vec<u8> {
    let strings = b"entry\0source.gltf\0props.ytyp\0mesh\0materials/test.nemat@test\0";
    let entry_name = 0u32;
    let source = 6u32;
    let properties = 18u32;
    let mesh_name = 29u32;
    let material = 34u32;
    let table_offset = BODY_HEADER_LEN;
    let string_offset = table_offset + ENTRY_RECORD_LEN;
    let payload_offset = string_offset + strings.len();
    let payload_len = 8 + MESH_HEADER_LEN + 3 * VERTEX_STRIDE + 3 * 4;
    let mut out = Vec::new();
    push_u32(&mut out, YDD_BINARY_SCHEMA_VERSION);
    push_u32(&mut out, 1);
    push_u64(&mut out, table_offset as u64);
    push_u64(&mut out, string_offset as u64);
    push_u64(&mut out, strings.len() as u64);
    push_u64(&mut out, payload_offset as u64);
    push_u64(&mut out, 1);
    push_u32(&mut out, entry_name);
    push_u32(&mut out, source);
    push_u32(&mut out, 1);
    push_u32(&mut out, 3);
    push_u32(&mut out, 3);
    push_u32(&mut out, 1);
    push_u32(&mut out, properties);
    push_vec3(&mut out, [0.0, 0.0, 0.0]);
    push_vec3(&mut out, [1.0, 0.0, 1.0]);
    push_u32(&mut out, 0);
    push_u64(&mut out, payload_offset as u64);
    push_u64(&mut out, payload_len as u64);
    out.extend_from_slice(strings);
    push_u32(&mut out, 1);
    push_u32(&mut out, 0);
    push_u32(&mut out, mesh_name);
    push_u32(&mut out, material);
    push_u32(&mut out, 3);
    push_u32(&mut out, 3);
    push_vec3(&mut out, [0.0, 0.0, 0.0]);
    push_vec3(&mut out, [1.0, 0.0, 1.0]);
    for (position, uv) in [
        ([0.0, 0.0, 0.0], [0.0, 0.0]),
        ([1.0, 0.0, 0.0], [1.0, 0.0]),
        ([0.0, 0.0, 1.0], [0.0, 1.0]),
    ] {
        push_vec3(&mut out, position);
        push_vec3(&mut out, [0.0, 1.0, 0.0]);
        push_f32(&mut out, uv[0]);
        push_f32(&mut out, uv[1]);
    }
    push_u32(&mut out, 0);
    push_u32(&mut out, 1);
    push_u32(&mut out, 2);
    out
}

#[test]
fn binary_ydd_decodes_strict_v2_body() {
    let document = decode_ydd_binary_body(&test_body()).expect("decode binary YDD");
    assert_eq!(document.entries.len(), 1);
    let entry = &document.entries[0];
    assert_eq!(entry.name, "entry");
    assert_eq!(entry.properties_ref.as_deref(), Some("props.ytyp"));
    assert_eq!(entry.meshes.len(), 1);
    assert_eq!(entry.meshes[0].vertices.len(), 3);
    assert_eq!(entry.meshes[0].indices, vec![0, 1, 2]);
    assert_eq!(entry.meshes[0].material_slot(), "test");
}

#[test]
fn binary_ydd_rejects_json_geometry() {
    let error =
        decode_ydd_binary_body(br#"{"runtime_mesh_parts":[]}"#).expect_err("JSON must be rejected");
    assert!(error.contains("JSON YDD geometry is unsupported"));
}
