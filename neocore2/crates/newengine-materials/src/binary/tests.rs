use super::{
    decode_asset, decode_descriptor, encode_asset, encode_descriptor, MaterialBinaryAsset,
    MaterialBinaryError, MATERIAL_BINARY_HEADER_SIZE, MATERIAL_BINARY_MAGIC,
    MATERIAL_BINARY_VERSION,
};
use crate::api::{MaterialDescriptor, MaterialDomain, MaterialFlags, ShadingModel};

fn sample_desc() -> MaterialDescriptor {
    MaterialDescriptor {
        domain: MaterialDomain::Surface,
        shading_model: ShadingModel::PbrMetallicRoughness,
        base_color: [0.25, 0.5, 0.75, 1.0],
        emissive: [0.2, 0.4, 0.6],
        emissive_strength: 3.5,
        metallic: 0.8,
        roughness: 0.35,
        normal_scale: 1.25,
        occlusion_strength: 0.9,
        alpha_cutoff: 0.42,
        flags: MaterialFlags::DOUBLE_SIDED.union(MaterialFlags::CAST_SHADOWS),
        reserved: [7, 11],
    }
}

#[test]
fn descriptor_roundtrip() {
    let desc = sample_desc();
    let bytes = encode_descriptor(&desc);
    let decoded = decode_descriptor(&bytes).expect("decode descriptor");
    assert_eq!(decoded, desc);
}

#[test]
fn asset_roundtrip() {
    let asset = MaterialBinaryAsset {
        name: "test_material".to_string(),
        desc: sample_desc(),
    };

    let bytes = encode_asset(&asset).expect("encode asset");
    let decoded = decode_asset(&bytes).expect("decode asset");
    assert_eq!(decoded, asset);
}

#[test]
fn rejects_bad_magic() {
    let mut bytes = encode_asset(&MaterialBinaryAsset {
        name: "m".to_string(),
        desc: sample_desc(),
    })
        .expect("encode asset");

    bytes[0] = b'X';
    let err = decode_asset(&bytes).expect_err("must reject bad magic");
    assert_eq!(err, MaterialBinaryError::InvalidMagic);
}

#[test]
fn rejects_bad_version() {
    let mut bytes = encode_asset(&MaterialBinaryAsset {
        name: "m".to_string(),
        desc: sample_desc(),
    })
        .expect("encode asset");

    bytes[8..10].copy_from_slice(&(MATERIAL_BINARY_VERSION + 1).to_le_bytes());
    let err = decode_asset(&bytes).expect_err("must reject bad version");
    assert_eq!(
        err,
        MaterialBinaryError::UnsupportedVersion {
            found: MATERIAL_BINARY_VERSION + 1,
        }
    );
}

#[test]
fn rejects_truncated_payload() {
    let mut bytes = encode_asset(&MaterialBinaryAsset {
        name: "m".to_string(),
        desc: sample_desc(),
    })
        .expect("encode asset");
    bytes.pop();

    let err = decode_asset(&bytes).expect_err("must reject truncated payload");
    assert_eq!(err, MaterialBinaryError::UnexpectedEof);
}

#[test]
fn rejects_invalid_descriptor_size() {
    let err = decode_descriptor(&[0u8; 4]).expect_err("must reject short descriptor");
    assert_eq!(
        err,
        MaterialBinaryError::InvalidDescriptorSize {
            found: 4,
            expected: super::MATERIAL_DESCRIPTOR_SIZE,
        }
    );
}

#[test]
fn header_layout_is_stable() {
    let bytes = encode_asset(&MaterialBinaryAsset {
        name: "m".to_string(),
        desc: sample_desc(),
    })
        .expect("encode asset");

    assert_eq!(&bytes[..8], &MATERIAL_BINARY_MAGIC);
    assert_eq!(bytes.len(), MATERIAL_BINARY_HEADER_SIZE + u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize);
}
