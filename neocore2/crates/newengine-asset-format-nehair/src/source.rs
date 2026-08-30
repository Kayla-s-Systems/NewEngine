use std::collections::BTreeMap;

use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;
use newengine_render_api::{
    HairCollisionCapsuleV1, HairGroomAssetV1, HairGroomRef, HairGuidePointV1, HairGuideStrandV1,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredHairGuidePointV1 {
    pub rest_position: [f32; 3],
    #[serde(default = "default_inverse_mass")]
    pub inverse_mass: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredHairGuideStrandV1 {
    pub first_point: u32,
    pub point_count: u16,
    #[serde(default)]
    pub group: u16,
    #[serde(default)]
    pub root_uv: [f32; 2],
    pub root_joint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredHairCollisionCapsuleV1 {
    pub joint: String,
    pub radius: f32,
    /// Endpoints authored in the joint's bind-local space.
    pub joint_local_a: [f32; 3],
    pub joint_local_b: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredHairGroomV1 {
    #[serde(default)]
    pub groom: Option<String>,
    #[serde(default)]
    pub guide_points: Vec<AuthoredHairGuidePointV1>,
    #[serde(default)]
    pub guide_strands: Vec<AuthoredHairGuideStrandV1>,
    #[serde(default)]
    pub collision_capsules: Vec<AuthoredHairCollisionCapsuleV1>,
    #[serde(default)]
    pub follow_strands_per_guide: u8,
}

#[inline]
fn default_inverse_mass() -> f32 {
    1.0
}

pub fn compile_authored_groom_json(
    bytes: &[u8],
    fallback_groom_ref: &str,
    skeleton: &ModelSkeletonMetadata,
) -> Result<HairGroomAssetV1, String> {
    let source: AuthoredHairGroomV1 = serde_json::from_slice(bytes)
        .map_err(|error| format!("NEHAIR authored JSON decode failed: {error}"))?;
    compile_authored_groom(source, fallback_groom_ref, skeleton)
}

pub fn compile_authored_groom(
    source: AuthoredHairGroomV1,
    fallback_groom_ref: &str,
    skeleton: &ModelSkeletonMetadata,
) -> Result<HairGroomAssetV1, String> {
    if skeleton.joints.is_empty() {
        return Err("NEHAIR compile requires a non-empty skeleton".to_owned());
    }
    if skeleton.joints.len() > u16::MAX as usize {
        return Err(format!(
            "NEHAIR skeleton has {} joints, exceeds u16 palette limit",
            skeleton.joints.len()
        ));
    }

    let lookup = joint_lookup(skeleton)?;
    let bind_globals = bind_global_matrices(skeleton, &lookup)?;
    let groom = source.groom.as_deref().unwrap_or(fallback_groom_ref);

    let guide_points = source
        .guide_points
        .into_iter()
        .map(|point| HairGuidePointV1 {
            rest_position: point.rest_position,
            inverse_mass: point.inverse_mass,
        })
        .collect::<Vec<_>>();

    let mut guide_strands = Vec::with_capacity(source.guide_strands.len());
    for strand in source.guide_strands {
        let root_joint_index = resolve_joint(&lookup, &strand.root_joint)?;
        guide_strands.push(HairGuideStrandV1 {
            first_point: strand.first_point,
            point_count: strand.point_count,
            group: strand.group,
            root_uv: strand.root_uv,
            root_joint_index,
        });
    }

    let mut collision_capsules = Vec::with_capacity(source.collision_capsules.len());
    for capsule in source.collision_capsules {
        let joint_index = resolve_joint(&lookup, &capsule.joint)?;
        let bind_global = bind_globals[joint_index as usize];
        let a = bind_global.transform_point3(Vec3::new(
            capsule.joint_local_a[0],
            capsule.joint_local_a[1],
            capsule.joint_local_a[2],
        ));
        let b = bind_global.transform_point3(Vec3::new(
            capsule.joint_local_b[0],
            capsule.joint_local_b[1],
            capsule.joint_local_b[2],
        ));
        collision_capsules.push(HairCollisionCapsuleV1 {
            joint_index,
            radius: capsule.radius,
            // Compiled NEHAIR stores model-space bind/rest endpoints. At runtime the
            // joint deformation matrix moves these points into the animated model pose.
            local_a: [a.x, a.y, a.z],
            local_b: [b.x, b.y, b.z],
        });
    }

    HairGroomAssetV1 {
        groom: HairGroomRef::new(groom),
        guide_points,
        guide_strands,
        collision_capsules,
        follow_strands_per_guide: source.follow_strands_per_guide,
    }
    .normalized()
}

fn joint_lookup(skeleton: &ModelSkeletonMetadata) -> Result<BTreeMap<String, u16>, String> {
    let mut lookup = BTreeMap::new();
    for (index, joint) in skeleton.joints.iter().enumerate() {
        let key = joint.name.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(format!("NEHAIR skeleton joint {index} has an empty name"));
        }
        if lookup.insert(key.clone(), index as u16).is_some() {
            return Err(format!(
                "NEHAIR skeleton has duplicate case-insensitive joint name '{}'",
                joint.name
            ));
        }
    }
    Ok(lookup)
}

fn resolve_joint(lookup: &BTreeMap<String, u16>, name: &str) -> Result<u16, String> {
    let key = name.trim().to_ascii_lowercase();
    lookup.get(&key).copied().ok_or_else(|| {
        format!(
            "NEHAIR authored joint '{}' does not exist in skeleton",
            name
        )
    })
}

fn bind_global_matrices(
    skeleton: &ModelSkeletonMetadata,
    lookup: &BTreeMap<String, u16>,
) -> Result<Vec<Mat4>, String> {
    let mut cache = vec![None; skeleton.joints.len()];
    let mut visiting = vec![false; skeleton.joints.len()];
    for index in 0..skeleton.joints.len() {
        resolve_bind_global(index, skeleton, lookup, &mut cache, &mut visiting)?;
    }
    Ok(cache
        .into_iter()
        .map(|matrix| matrix.expect("all bind globals resolved"))
        .collect())
}

fn resolve_bind_global(
    index: usize,
    skeleton: &ModelSkeletonMetadata,
    lookup: &BTreeMap<String, u16>,
    cache: &mut [Option<Mat4>],
    visiting: &mut [bool],
) -> Result<Mat4, String> {
    if let Some(matrix) = cache[index] {
        return Ok(matrix);
    }
    if visiting[index] {
        return Err(format!(
            "NEHAIR skeleton parent cycle detected at joint '{}'",
            skeleton.joints[index].name
        ));
    }
    visiting[index] = true;
    let joint = &skeleton.joints[index];
    let rotation = Quat::from_xyzw(
        joint.rotation_ls[0],
        joint.rotation_ls[1],
        joint.rotation_ls[2],
        joint.rotation_ls[3],
    )
    .normalize_or_identity();
    let local = Mat4::from_scale_rotation_translation(
        Vec3::new(joint.scale_ls[0], joint.scale_ls[1], joint.scale_ls[2]),
        rotation,
        Vec3::new(
            joint.position_ls[0],
            joint.position_ls[1],
            joint.position_ls[2],
        ),
    );
    let parent_index = match joint.parent_index {
        Some(parent) => Some(parent as usize),
        None => joint
            .parent
            .as_deref()
            .map(|parent| resolve_joint(lookup, parent).map(usize::from))
            .transpose()?,
    };
    let global = if let Some(parent) = parent_index {
        if parent >= skeleton.joints.len() {
            return Err(format!(
                "NEHAIR joint '{}' parent index {} outside skeleton",
                joint.name, parent
            ));
        }
        resolve_bind_global(parent, skeleton, lookup, cache, visiting)? * local
    } else {
        local
    };
    visiting[index] = false;
    cache[index] = Some(global);
    Ok(global)
}
