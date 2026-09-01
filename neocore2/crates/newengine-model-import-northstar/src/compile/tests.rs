use super::*;

fn skin(joints: [u16; 4], weights: [f32; 4]) -> YddBinarySkinVertex {
    YddBinarySkinVertex {
        joints,
        weights,
        joints_extra: [0; 4],
        weights_extra: [0.0; 4],
    }
}

#[test]
fn master_rig_skin_passthrough_is_exact() {
    let input = skin([53, 29, 0, 0], [0.7, 0.3, 0.0, 0.0]);
    let output =
        remap_skin_vertex_to_master(input, 62, 62, None).expect("master-domain passthrough");
    assert_eq!(output, input);
}

#[test]
fn subset_remap_changes_only_weighted_joint_indices() {
    let input = skin([22, 5, 0, 0], [0.75, 0.25, 0.0, 0.0]);
    let mut mapping = vec![None; 35];
    mapping[22] = Some(53);
    mapping[5] = Some(29);
    let output = remap_skin_vertex_to_master(input, 35, 62, Some(&mapping)).expect("subset remap");
    assert_eq!(output.joints, [53, 29, 0, 0]);
    assert_eq!(output.weights, input.weights);
    assert_eq!(output.joints_extra, input.joints_extra);
    assert_eq!(output.weights_extra, input.weights_extra);
}

#[test]
fn subset_remap_requires_every_weighted_local_joint() {
    let input = skin([22, 8, 0, 0], [0.5, 0.5, 0.0, 0.0]);
    let mut mapping = vec![None; 35];
    mapping[22] = Some(53);
    let error = remap_skin_vertex_to_master(input, 35, 62, Some(&mapping))
        .expect_err("missing local mapping must reject");
    assert!(error.contains("local joint 8"), "{error}");
}

#[test]
fn unknown_source_skin_domain_is_rejected() {
    let input = skin([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]);
    let error =
        remap_skin_vertex_to_master(input, 41, 62, None).expect_err("unknown domain must reject");
    assert!(error.contains("unknown skin domain"), "{error}");
}

#[test]
fn zero_weight_joint_does_not_require_subset_mapping() {
    let input = skin([22, 34, 0, 0], [1.0, 0.0, 0.0, 0.0]);
    let mut mapping = vec![None; 35];
    mapping[22] = Some(53);
    let output = remap_skin_vertex_to_master(input, 35, 62, Some(&mapping))
        .expect("zero-weight slot is irrelevant");
    assert_eq!(output.joints[0], 53);
    assert_eq!(output.weights, input.weights);
}
