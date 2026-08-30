#![forbid(unsafe_op_in_unsafe_fn)]

mod binary;
mod source;

pub use binary::{decode_nehair, encode_nehair_v1, NEHAIR_MAGIC, NEHAIR_VERSION_V1};
pub use source::{
    compile_authored_groom, compile_authored_groom_json, AuthoredHairCollisionCapsuleV1,
    AuthoredHairGroomV1, AuthoredHairGuidePointV1, AuthoredHairGuideStrandV1,
};

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_model_skeleton_api::{
        ModelSkeletonAnchors, ModelSkeletonJointMetadata, ModelSkeletonMetadata,
    };

    fn skeleton() -> ModelSkeletonMetadata {
        ModelSkeletonMetadata {
            source: "test".to_owned(),
            source_format: "unit".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: "test".to_owned(),
            decode_status: "ok".to_owned(),
            joints: vec![
                ModelSkeletonJointMetadata {
                    index: 0,
                    tag: 0,
                    name: "root".to_owned(),
                    parent: None,
                    parent_index: None,
                    position_ls: [0.0, 0.0, 0.0],
                    rotation_ls: [0.0, 0.0, 0.0, 1.0],
                    scale_ls: [1.0; 3],
                    flags: Vec::new(),
                },
                ModelSkeletonJointMetadata {
                    index: 1,
                    tag: 0,
                    name: "Head".to_owned(),
                    parent: Some("root".to_owned()),
                    parent_index: Some(0),
                    position_ls: [0.0, 1.5, 0.0],
                    rotation_ls: [0.0, 0.0, 0.0, 1.0],
                    scale_ls: [1.0; 3],
                    flags: Vec::new(),
                },
            ],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "Head".to_owned(),
                left_hand: String::new(),
                right_hand: String::new(),
                left_foot: String::new(),
                right_foot: String::new(),
                eye: "Head".to_owned(),
                eye_height: 1.6,
            },
        }
    }

    fn authored_json() -> Vec<u8> {
        br#"{
          "groom":"characters/test/head_hair.nehair",
          "guide_points":[
            {"rest_position":[0.0,1.55,0.0],"inverse_mass":0.0},
            {"rest_position":[0.0,1.45,0.0],"inverse_mass":1.0},
            {"rest_position":[0.0,1.35,0.0],"inverse_mass":1.0}
          ],
          "guide_strands":[
            {"first_point":0,"point_count":3,"root_joint":"head","root_uv":[0.5,0.5]}
          ],
          "collision_capsules":[
            {"joint":"HEAD","radius":0.1,"joint_local_a":[0.0,0.0,0.0],"joint_local_b":[0.0,0.2,0.0]}
          ],
          "follow_strands_per_guide":4
        }"#
        .to_vec()
    }

    #[test]
    fn authored_compiler_resolves_joint_names_case_insensitively() {
        let groom = compile_authored_groom_json(
            &authored_json(),
            "characters/fallback.nehair",
            &skeleton(),
        )
        .unwrap();
        assert_eq!(groom.guide_strands[0].root_joint_index, 1);
        assert_eq!(groom.collision_capsules[0].joint_index, 1);
        assert!((groom.collision_capsules[0].local_a[1] - 1.5).abs() < 1.0e-5);
        assert!((groom.collision_capsules[0].local_b[1] - 1.7).abs() < 1.0e-5);
    }

    #[test]
    fn binary_roundtrip_preserves_compiled_groom() {
        let groom = compile_authored_groom_json(
            &authored_json(),
            "characters/fallback.nehair",
            &skeleton(),
        )
        .unwrap();
        let bytes = encode_nehair_v1(&groom).unwrap();
        assert_eq!(&bytes[..8], &NEHAIR_MAGIC);
        let decoded = decode_nehair(&bytes).unwrap();
        assert_eq!(decoded, groom);
    }

    #[test]
    fn binary_digest_detects_corruption() {
        let groom = compile_authored_groom_json(
            &authored_json(),
            "characters/fallback.nehair",
            &skeleton(),
        )
        .unwrap();
        let mut bytes = encode_nehair_v1(&groom).unwrap();
        *bytes.last_mut().unwrap() ^= 0x1;
        assert!(decode_nehair(&bytes).unwrap_err().contains("digest"));
    }

    #[test]
    fn compiler_rejects_unknown_root_joint() {
        let source = AuthoredHairGroomV1 {
            groom: None,
            guide_points: vec![
                AuthoredHairGuidePointV1 {
                    rest_position: [0.0, 0.0, 0.0],
                    inverse_mass: 0.0,
                },
                AuthoredHairGuidePointV1 {
                    rest_position: [0.0, -0.1, 0.0],
                    inverse_mass: 1.0,
                },
            ],
            guide_strands: vec![AuthoredHairGuideStrandV1 {
                first_point: 0,
                point_count: 2,
                group: 0,
                root_uv: [0.0, 0.0],
                root_joint: "missing_joint".to_owned(),
            }],
            collision_capsules: Vec::new(),
            follow_strands_per_guide: 0,
        };
        assert!(
            compile_authored_groom(source, "characters/test.nehair", &skeleton())
                .unwrap_err()
                .contains("does not exist")
        );
    }
}
