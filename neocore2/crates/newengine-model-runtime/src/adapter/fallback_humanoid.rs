pub(super) fn default_humanoid_joints(
    target_height: f32,
) -> Vec<newengine_model_skeleton_api::ModelSkeletonJointMetadata> {
    use newengine_model_skeleton_api::skeleton_joint_indexed;
    vec![
        skeleton_joint_indexed(0, 0, "root", Option::<String>::None, None, [0.0, 0.0, 0.0]),
        skeleton_joint_indexed(
            1,
            0,
            "hips",
            Some("root"),
            Some(0),
            [0.0, target_height * 0.50, 0.0],
        ),
        skeleton_joint_indexed(
            2,
            0,
            "spine",
            Some("hips"),
            Some(1),
            [0.0, target_height * 0.18, 0.0],
        ),
        skeleton_joint_indexed(
            3,
            0,
            "head",
            Some("spine"),
            Some(2),
            [0.0, target_height * 0.23, 0.0],
        ),
        skeleton_joint_indexed(
            4,
            0,
            "left_hand",
            Some("spine"),
            Some(2),
            [-0.42, -target_height * 0.10, 0.0],
        ),
        skeleton_joint_indexed(
            5,
            0,
            "right_hand",
            Some("spine"),
            Some(2),
            [0.42, -target_height * 0.10, 0.0],
        ),
        skeleton_joint_indexed(
            6,
            0,
            "left_foot",
            Some("hips"),
            Some(1),
            [-0.16, -target_height * 0.48, 0.0],
        ),
        skeleton_joint_indexed(
            7,
            0,
            "right_foot",
            Some("hips"),
            Some(1),
            [0.16, -target_height * 0.48, 0.0],
        ),
        skeleton_joint_indexed(8, 0, "eye", Some("head"), Some(3), [0.0, 0.0, -0.08]),
    ]
}

pub(super) fn default_humanoid_anchors(
    target_height: f32,
    eye_height_ratio: f32,
) -> newengine_model_skeleton_api::ModelSkeletonAnchors {
    newengine_model_skeleton_api::ModelSkeletonAnchors {
        root: "root".to_owned(),
        hips: "hips".to_owned(),
        head: "head".to_owned(),
        left_hand: "left_hand".to_owned(),
        right_hand: "right_hand".to_owned(),
        left_foot: "left_foot".to_owned(),
        right_foot: "right_foot".to_owned(),
        eye: "eye".to_owned(),
        eye_height: target_height * eye_height_ratio.clamp(0.55, 0.98),
    }
}
