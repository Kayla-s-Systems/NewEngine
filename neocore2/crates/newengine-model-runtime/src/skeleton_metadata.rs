use std::collections::{BTreeMap, BTreeSet};

use newengine_authored_xml::{parse_xml_body, XmlNode};
use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

pub(crate) struct DecodedSkeletonBody {
    pub source_format: String,
    pub decode_status: String,
    pub joints: Vec<ModelSkeletonJointMetadata>,
    pub anchors: ModelSkeletonAnchors,
}

pub(crate) fn decode_skeleton_body(
    body: &[u8],
    target_height: f32,
    eye_height_ratio: f32,
) -> Result<Option<DecodedSkeletonBody>, String> {
    let document = parse_xml_body(body, "model skeleton metadata")?;
    let root = document.root_element();
    let Some(skeleton) = root
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("Skeleton"))
    else {
        return Ok(None);
    };

    let mut joints = skeleton
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("Joint"))
        .map(parse_joint)
        .collect::<Result<Vec<_>, _>>()?;
    if joints.is_empty() {
        return Err(
            "model skeleton metadata contains <Skeleton> but no <Joint> records".to_owned(),
        );
    }

    joints.sort_by_key(|joint| joint.index);
    validate_and_resolve_parents(&mut joints)?;

    let anchors = skeleton
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("Anchors"))
        .map(|node| parse_anchors(node, &joints, target_height, eye_height_ratio))
        .transpose()?
        .unwrap_or_else(|| derive_anchors(&joints, target_height, eye_height_ratio));
    validate_anchors(&anchors, &joints)?;

    let source_format = skeleton
        .attribute("source_format")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("newengine.ymt.skeleton.v1")
        .to_owned();
    let decode_status = format!(
        "decoded authored bind-pose skeleton joints={} source_format='{}'",
        joints.len(),
        source_format
    );

    Ok(Some(DecodedSkeletonBody {
        source_format,
        decode_status,
        joints,
        anchors,
    }))
}

fn parse_joint(node: XmlNode<'_, '_>) -> Result<ModelSkeletonJointMetadata, String> {
    let name = required_attr(node, "name")?.to_owned();
    let index = parse_required::<u32>(node, "index")?;
    let tag = parse_optional::<u32>(node, "tag")?.unwrap_or(0);
    let parent = optional_attr(node, "parent").map(ToOwned::to_owned);
    let parent_index = match parse_optional::<i32>(node, "parent_index")? {
        Some(value) if value >= 0 => Some(value as u32),
        _ => None,
    };
    let position_ls = [
        parse_required::<f32>(node, "tx")?,
        parse_required::<f32>(node, "ty")?,
        parse_required::<f32>(node, "tz")?,
    ];
    let rotation_ls = [
        parse_optional::<f32>(node, "qx")?.unwrap_or(0.0),
        parse_optional::<f32>(node, "qy")?.unwrap_or(0.0),
        parse_optional::<f32>(node, "qz")?.unwrap_or(0.0),
        parse_optional::<f32>(node, "qw")?.unwrap_or(1.0),
    ];
    let scale_ls = [
        parse_optional::<f32>(node, "sx")?.unwrap_or(1.0),
        parse_optional::<f32>(node, "sy")?.unwrap_or(1.0),
        parse_optional::<f32>(node, "sz")?.unwrap_or(1.0),
    ];
    let flags = optional_attr(node, "flags")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !position_ls.iter().all(|value| value.is_finite())
        || !rotation_ls.iter().all(|value| value.is_finite())
        || !scale_ls.iter().all(|value| value.is_finite())
    {
        return Err(format!(
            "skeleton joint '{name}' contains non-finite bind-pose values"
        ));
    }

    Ok(ModelSkeletonJointMetadata {
        index,
        tag,
        name,
        parent,
        parent_index,
        position_ls,
        rotation_ls,
        scale_ls,
        flags,
    })
}

fn validate_and_resolve_parents(joints: &mut [ModelSkeletonJointMetadata]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    let mut indices = BTreeMap::new();
    for joint in joints.iter() {
        if !names.insert(joint.name.clone()) {
            return Err(format!("duplicate skeleton joint name '{}'", joint.name));
        }
        if indices.insert(joint.index, joint.name.clone()).is_some() {
            return Err(format!("duplicate skeleton joint index {}", joint.index));
        }
    }

    for joint in joints.iter_mut() {
        if let Some(parent_index) = joint.parent_index {
            if parent_index == joint.index {
                return Err(format!("skeleton joint '{}' parents itself", joint.name));
            }
            let indexed_parent = indices.get(&parent_index).ok_or_else(|| {
                format!(
                    "skeleton joint '{}' references missing parent index {}",
                    joint.name, parent_index
                )
            })?;
            if let Some(parent) = joint.parent.as_deref() {
                if parent != indexed_parent {
                    return Err(format!(
                        "skeleton joint '{}' parent mismatch name='{}' index={} resolves='{}'",
                        joint.name, parent, parent_index, indexed_parent
                    ));
                }
            } else {
                joint.parent = Some(indexed_parent.clone());
            }
        } else if let Some(parent) = joint.parent.as_deref() {
            if !names.contains(parent) {
                return Err(format!(
                    "skeleton joint '{}' references missing parent '{}'",
                    joint.name, parent
                ));
            }
        }
    }
    Ok(())
}

fn parse_anchors(
    node: XmlNode<'_, '_>,
    joints: &[ModelSkeletonJointMetadata],
    target_height: f32,
    eye_height_ratio: f32,
) -> Result<ModelSkeletonAnchors, String> {
    let derived = derive_anchors(joints, target_height, eye_height_ratio);
    Ok(ModelSkeletonAnchors {
        root: optional_attr(node, "root")
            .unwrap_or(&derived.root)
            .to_owned(),
        hips: optional_attr(node, "hips")
            .unwrap_or(&derived.hips)
            .to_owned(),
        head: optional_attr(node, "head")
            .unwrap_or(&derived.head)
            .to_owned(),
        left_hand: optional_attr(node, "left_hand")
            .unwrap_or(&derived.left_hand)
            .to_owned(),
        right_hand: optional_attr(node, "right_hand")
            .unwrap_or(&derived.right_hand)
            .to_owned(),
        left_foot: optional_attr(node, "left_foot")
            .unwrap_or(&derived.left_foot)
            .to_owned(),
        right_foot: optional_attr(node, "right_foot")
            .unwrap_or(&derived.right_foot)
            .to_owned(),
        eye: optional_attr(node, "eye")
            .unwrap_or(&derived.eye)
            .to_owned(),
        eye_height: parse_optional::<f32>(node, "eye_height")?
            .unwrap_or(target_height * eye_height_ratio.clamp(0.55, 0.98)),
    })
}

fn derive_anchors(
    joints: &[ModelSkeletonJointMetadata],
    target_height: f32,
    eye_height_ratio: f32,
) -> ModelSkeletonAnchors {
    ModelSkeletonAnchors {
        root: pick_joint(joints, &["SKEL_ROOT", "root"]),
        hips: pick_joint(joints, &["SKEL_Pelvis", "hips", "pelvis"]),
        head: pick_joint(joints, &["SKEL_Head", "head"]),
        left_hand: pick_joint(joints, &["SKEL_L_Hand", "left_hand"]),
        right_hand: pick_joint(joints, &["SKEL_R_Hand", "right_hand"]),
        left_foot: pick_joint(joints, &["SKEL_L_Foot", "left_foot"]),
        right_foot: pick_joint(joints, &["SKEL_R_Foot", "right_foot"]),
        eye: pick_joint(
            joints,
            &["FACIAL_L_eyeball", "FACIAL_R_eyeball", "SKEL_Head", "eye"],
        ),
        eye_height: target_height * eye_height_ratio.clamp(0.55, 0.98),
    }
}

fn validate_anchors(
    anchors: &ModelSkeletonAnchors,
    joints: &[ModelSkeletonJointMetadata],
) -> Result<(), String> {
    let names = joints
        .iter()
        .map(|joint| joint.name.as_str())
        .collect::<BTreeSet<_>>();
    for (label, value) in [
        ("root", anchors.root.as_str()),
        ("hips", anchors.hips.as_str()),
        ("head", anchors.head.as_str()),
        ("left_hand", anchors.left_hand.as_str()),
        ("right_hand", anchors.right_hand.as_str()),
        ("left_foot", anchors.left_foot.as_str()),
        ("right_foot", anchors.right_foot.as_str()),
        ("eye", anchors.eye.as_str()),
    ] {
        if !names.contains(value) {
            return Err(format!(
                "skeleton anchor '{label}' references missing joint '{value}'"
            ));
        }
    }
    Ok(())
}

fn pick_joint(joints: &[ModelSkeletonJointMetadata], candidates: &[&str]) -> String {
    candidates
        .iter()
        .find_map(|candidate| {
            joints
                .iter()
                .find(|joint| joint.name.eq_ignore_ascii_case(candidate))
                .map(|joint| joint.name.clone())
        })
        .or_else(|| joints.first().map(|joint| joint.name.clone()))
        .unwrap_or_else(|| "root".to_owned())
}

fn required_attr<'a>(node: XmlNode<'a, '_>, name: &str) -> Result<&'a str, String> {
    optional_attr(node, name).ok_or_else(|| {
        format!(
            "skeleton <{}> missing required '{}' attribute",
            node.tag_name().name(),
            name
        )
    })
}

fn optional_attr<'a>(node: XmlNode<'a, '_>, name: &str) -> Option<&'a str> {
    node.attribute(name)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_required<T>(node: XmlNode<'_, '_>, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    required_attr(node, name)?.parse::<T>().map_err(|error| {
        format!(
            "skeleton <{}> invalid '{}' value: {}",
            node.tag_name().name(),
            name,
            error
        )
    })
}

fn parse_optional<T>(node: XmlNode<'_, '_>, name: &str) -> Result<Option<T>, String>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    optional_attr(node, name)
        .map(|value| {
            value.parse::<T>().map_err(|error| {
                format!(
                    "skeleton <{}> invalid '{}' value: {}",
                    node.tag_name().name(),
                    name,
                    error
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_rage_bind_pose_projection() {
        let body = br#"<YmtMetadata><Entry><Skeleton source_format="rage.yft.xml"><Joint index="0" tag="0" name="SKEL_ROOT" parent_index="-1" tx="0" ty="0" tz="0" qx="0" qy="0" qz="0" qw="1" sx="1" sy="1" sz="1" flags="RotX,RotY"/><Joint index="1" tag="11816" name="SKEL_Pelvis" parent="SKEL_ROOT" parent_index="0" tx="0" ty="0" tz="0"/><Joint index="2" name="SKEL_Head" parent="SKEL_Pelvis" parent_index="1" tx="0" ty="1" tz="0"/><Joint index="3" name="SKEL_L_Hand" parent="SKEL_Pelvis" parent_index="1" tx="0" ty="0" tz="0"/><Joint index="4" name="SKEL_R_Hand" parent="SKEL_Pelvis" parent_index="1" tx="0" ty="0" tz="0"/><Joint index="5" name="SKEL_L_Foot" parent="SKEL_Pelvis" parent_index="1" tx="0" ty="0" tz="0"/><Joint index="6" name="SKEL_R_Foot" parent="SKEL_Pelvis" parent_index="1" tx="0" ty="0" tz="0"/><Joint index="7" name="FACIAL_L_eyeball" parent="SKEL_Head" parent_index="2" tx="0" ty="0" tz="0"/><Anchors root="SKEL_ROOT" hips="SKEL_Pelvis" head="SKEL_Head" left_hand="SKEL_L_Hand" right_hand="SKEL_R_Hand" left_foot="SKEL_L_Foot" right_foot="SKEL_R_Foot" eye="FACIAL_L_eyeball"/></Skeleton></Entry></YmtMetadata>"#;
        let decoded = decode_skeleton_body(body, 1.78, 0.91)
            .expect("decode")
            .expect("skeleton");
        assert_eq!(decoded.joints.len(), 8);
        assert_eq!(decoded.joints[1].parent.as_deref(), Some("SKEL_ROOT"));
        assert_eq!(decoded.anchors.eye, "FACIAL_L_eyeball");
    }
}
