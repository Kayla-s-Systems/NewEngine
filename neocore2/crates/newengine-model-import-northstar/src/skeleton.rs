use newengine_math::{Mat4, Quat, Vec3, Vec4};

use crate::pak::PakFile;

#[derive(Clone, Debug)]
pub struct ImportedJoint {
    pub index: u32,
    pub tag: u32,
    pub name: String,
    pub parent_index: Option<u32>,
    pub position_ls: [f32; 3],
    pub rotation_ls: [f32; 4],
    pub scale_ls: [f32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkeletonProfile {
    Humanoid,
    Generic,
}

#[derive(Clone, Debug)]
pub struct DecodedSkeleton {
    pub name: String,
    pub joints: Vec<ImportedJoint>,
    pub root: String,
    pub hips: String,
    pub head: String,
    pub left_hand: String,
    pub right_hand: String,
    pub left_foot: String,
    pub right_foot: String,
    pub eye: String,
    pub eye_height: f32,
}

pub fn decode_skeleton(pak: &PakFile) -> Result<DecodedSkeleton, String> {
    decode_skeleton_with_profile(pak, SkeletonProfile::Humanoid)
}

pub fn decode_skeleton_with_profile(
    pak: &PakFile,
    profile: SkeletonProfile,
) -> Result<DecodedSkeleton, String> {
    let resource = pak
        .resource("JOINT_HIERARCHY")
        .ok_or_else(|| "package contains no JOINT_HIERARCHY resource".to_owned())?;
    let payload = pak.resource_payload(resource)?;
    let node_count = pak.read_u32(payload + 20)? as usize;
    if node_count == 0 || node_count > 4096 {
        return Err(format!(
            "invalid North Star skeleton node count {node_count}"
        ));
    }
    let joints_info = pak
        .resolve_pointer(payload + 32)?
        .ok_or_else(|| "JOINT_HIERARCHY has no matrix info pointer".to_owned())?;
    let names = pak
        .resolve_pointer(payload + 56)?
        .ok_or_else(|| "JOINT_HIERARCHY has no names pointer".to_owned())?;
    let ji_node_count = pak.read_u16(joints_info + 16)? as usize;
    if ji_node_count != node_count {
        return Err(format!(
            "skeleton node count mismatch hierarchy={node_count} matrices={ji_node_count}"
        ));
    }
    let matrix_relative = pak.read_u32(joints_info + 40)? as usize;
    let parenting_relative = pak.read_u32(joints_info + 60)? as usize;
    let matrix_start = joints_info
        .checked_add(matrix_relative)
        .ok_or("skeleton matrix table overflow")?;
    let parenting_header = joints_info
        .checked_add(parenting_relative)
        .ok_or("skeleton parenting header overflow")?;
    let parent_records_relative = pak.read_u32(parenting_header + 20)? as usize;
    let parent_start = parenting_header
        .checked_add(parent_records_relative)
        .ok_or("skeleton parenting records overflow")?;

    let mut node_names = Vec::with_capacity(node_count);
    let mut parents = Vec::with_capacity(node_count);
    let mut globals = Vec::with_capacity(node_count);
    for index in 0..node_count {
        let name_relative = pak.read_u64(names + index * 16 + 8)? as usize;
        let name = pak.string_at(
            resource
                .page_start
                .checked_add(name_relative)
                .ok_or("skeleton name address overflow")?,
        )?;
        if name.trim().is_empty() {
            return Err(format!("skeleton node {index} has empty name"));
        }
        node_names.push(name);

        let parent = pak.read_i32(parent_start + index * 16 + 4)?;
        if parent >= node_count as i32 {
            return Err(format!(
                "skeleton node {index} parent outside node table parent={parent} nodes={node_count}"
            ));
        }
        parents.push((parent >= 0).then_some(parent as u32));

        let matrix = read_inverse_global_matrix(pak, matrix_start + index * 48)?;
        let global = matrix.inverse();
        if global
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "skeleton node {index} produced non-finite global bind matrix"
            ));
        }
        globals.push(global);
    }
    let mut unique = std::collections::BTreeSet::new();
    for name in &node_names {
        if !unique.insert(name.as_str()) {
            return Err(format!("skeleton contains duplicate node name '{name}'"));
        }
    }

    let mut joints = Vec::with_capacity(node_count);
    for index in 0..node_count {
        let local = parents[index]
            .map(|parent| globals[parent as usize].inverse() * globals[index])
            .unwrap_or(globals[index]);
        let (scale, rotation, translation) = local.to_scale_rotation_translation();
        let rotation = rotation.normalize_or_identity();
        if !scale.is_finite() || !translation.is_finite() || !rotation.is_finite() {
            return Err(format!(
                "skeleton node {index} has non-finite local bind transform"
            ));
        }
        joints.push(ImportedJoint {
            index: index as u32,
            // Dense node index is the stable source identity used by packed skin weights.
            tag: index as u32,
            name: node_names[index].clone(),
            parent_index: parents[index],
            position_ls: [translation.x, translation.y, translation.z],
            rotation_ls: [rotation.x, rotation.y, rotation.z, rotation.w],
            scale_ls: [scale.x, scale.y, scale.z],
        });
    }

    validate_bind_reconstruction(&joints, &globals)?;

    let (root, hips, head, left_hand, right_hand, left_foot, right_foot, eye, eye_height) =
        match profile {
            SkeletonProfile::Humanoid => {
                let root = required_anchor(&node_names, &["root"])?;
                let hips = required_anchor(&node_names, &["pelvis"])?;
                let head = required_anchor(&node_names, &["headb", "heada"])?;
                let left_hand = required_anchor(&node_names, &["l_wrist", "l_palm"])?;
                let right_hand = required_anchor(&node_names, &["r_wrist", "r_palm"])?;
                let left_foot = required_anchor(&node_names, &["l_ankle", "l_ball"])?;
                let right_foot = required_anchor(&node_names, &["r_ankle", "r_ball"])?;
                let eye = required_anchor(&node_names, &["l_eyeball", "r_eyeball", "headb"])?;
                let eye_index = node_names
                    .iter()
                    .position(|name| name == &eye)
                    .ok_or("eye anchor disappeared")?;
                let eye_height = globals[eye_index]
                    .transform_point3(newengine_math::Vec3::ZERO)
                    .y;
                (
                    root, hips, head, left_hand, right_hand, left_foot, right_foot, eye, eye_height,
                )
            }
            SkeletonProfile::Generic => {
                let root_index = parents.iter().position(Option::is_none).unwrap_or(0);
                let root = node_names[root_index].clone();
                (
                    root.clone(),
                    root.clone(),
                    root.clone(),
                    root.clone(),
                    root.clone(),
                    root.clone(),
                    root.clone(),
                    root,
                    0.0,
                )
            }
        };

    Ok(DecodedSkeleton {
        name: resource.name.clone(),
        joints,
        root,
        hips,
        head,
        left_hand,
        right_hand,
        left_foot,
        right_foot,
        eye,
        eye_height,
    })
}

fn validate_bind_reconstruction(
    joints: &[ImportedJoint],
    source_globals: &[Mat4],
) -> Result<(), String> {
    if joints.len() != source_globals.len() {
        return Err("skeleton bind validation count mismatch".to_owned());
    }
    let mut reconstructed = vec![Mat4::IDENTITY; joints.len()];
    let mut max_error = 0.0_f32;
    let mut max_joint = 0usize;
    for (index, joint) in joints.iter().enumerate() {
        let local = Mat4::from_scale_rotation_translation(
            Vec3::new(joint.scale_ls[0], joint.scale_ls[1], joint.scale_ls[2]),
            Quat::from_xyzw(
                joint.rotation_ls[0],
                joint.rotation_ls[1],
                joint.rotation_ls[2],
                joint.rotation_ls[3],
            )
            .normalize_or_identity(),
            Vec3::new(
                joint.position_ls[0],
                joint.position_ls[1],
                joint.position_ls[2],
            ),
        );
        reconstructed[index] = joint
            .parent_index
            .map(|parent| reconstructed[parent as usize] * local)
            .unwrap_or(local);
        let actual = reconstructed[index].to_cols_array();
        let expected = source_globals[index].to_cols_array();
        let error = actual
            .iter()
            .zip(expected.iter())
            .map(|(actual, expected)| (actual - expected).abs())
            .fold(0.0_f32, f32::max);
        if error > max_error {
            max_error = error;
            max_joint = index;
        }
    }
    // The source affine matrices for Abby/TLOU2 PC are representable as S/R/T to
    // floating-point precision. Reject future packages that would silently lose
    // shear/non-SRT bind data in the current YMT contract.
    const MAX_BIND_RECONSTRUCTION_ERROR: f32 = 1.0e-5;
    if !max_error.is_finite() || max_error > MAX_BIND_RECONSTRUCTION_ERROR {
        return Err(format!(
            "skeleton bind transform is not losslessly representable by native YMT joint SRT max_error={max_error:.8} joint={} name='{}' limit={MAX_BIND_RECONSTRUCTION_ERROR}",
            max_joint,
            joints[max_joint].name,
        ));
    }
    Ok(())
}

fn read_inverse_global_matrix(pak: &PakFile, at: usize) -> Result<Mat4, String> {
    let mut value = [0.0f32; 12];
    for (index, component) in value.iter_mut().enumerate() {
        *component = pak.read_f32(at + index * 4)?;
    }
    // Source stores three row-major rows of an inverse global affine matrix.
    Ok(Mat4::from_cols(
        Vec4::new(value[0], value[4], value[8], 0.0),
        Vec4::new(value[1], value[5], value[9], 0.0),
        Vec4::new(value[2], value[6], value[10], 0.0),
        Vec4::new(value[3], value[7], value[11], 1.0),
    ))
}

fn required_anchor(names: &[String], candidates: &[&str]) -> Result<String, String> {
    candidates
        .iter()
        .find(|candidate| names.iter().any(|name| name == **candidate))
        .map(|candidate| (*candidate).to_owned())
        .ok_or_else(|| format!("skeleton is missing required anchor candidates={candidates:?}"))
}
