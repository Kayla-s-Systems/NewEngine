use super::*;

use newengine_math::Mat4;
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[inline]
fn normalized_ref(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

#[inline]
fn is_abby_ref(value: &str) -> bool {
    let value = normalized_ref(value);
    value.contains("/characters/abby/") || value.contains("@abby") || value.ends_with("/abby")
}

pub(super) fn validate_player_asset_family(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
) -> Result<(), String> {
    if !is_abby_ref(&assignment.source) {
        return Ok(());
    }
    let mut refs = Vec::<(&str, &str, bool)>::new();
    refs.push((
        "skeleton",
        assignment.skeleton_source.as_deref().unwrap_or_default(),
        true,
    ));
    for (role, reference) in [
        ("idle_animation", assignment.idle_animation.as_deref()),
        ("walk_animation", assignment.walk_animation.as_deref()),
        ("run_animation", assignment.run_animation.as_deref()),
        ("sprint_animation", assignment.sprint_animation.as_deref()),
        ("jump_animation", assignment.jump_animation.as_deref()),
        ("fall_animation", assignment.fall_animation.as_deref()),
    ] {
        if let Some(reference) = reference {
            refs.push((role, reference, false));
        }
    }
    if let Some(properties_ref) = assignment.properties_ref.as_deref() {
        refs.push(("properties", properties_ref, false));
    }

    for (role, reference, required) in refs {
        let normalized = normalized_ref(reference);
        if required && normalized.is_empty() {
            return Err(format!(
                "Abby asset chain requires authored {role}; model='{}'",
                assignment.source
            ));
        }
        if normalized.is_empty() {
            continue;
        }
        if normalized.contains("abigail") {
            return Err(format!(
                "Abby asset chain rejected foreign Abigail reference role={role} ref='{reference}'"
            ));
        }
        if matches!(
            role,
            "skeleton"
                | "idle_animation"
                | "walk_animation"
                | "run_animation"
                | "sprint_animation"
                | "jump_animation"
                | "fall_animation"
        ) && !is_abby_ref(reference)
        {
            return Err(format!(
                "Abby asset chain requires Abby-owned reference role={role} ref='{reference}'"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_player_skin_contract(
    parts: &[PlayerRuntimeModelPart],
    skeleton: Option<&ModelSkeletonMetadata>,
) -> Result<Option<[f32; 16]>, String> {
    const WEIGHT_EPSILON: f32 = 1.0e-3;
    let skinned = parts
        .iter()
        .enumerate()
        .filter_map(|(part_index, part)| part.skin.as_ref().map(|skin| (part_index, skin)))
        .collect::<Vec<_>>();
    if skinned.is_empty() {
        return Ok(None);
    }
    let skeleton = skeleton
        .ok_or_else(|| "skinned player model requires authored skeleton metadata".to_owned())?;
    let joint_count = skeleton.joints.len();
    if joint_count == 0 || joint_count > 4096 {
        return Err(format!(
            "skinned player skeleton joint count outside runtime range joints={joint_count} supported=1..=4096"
        ));
    }
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if joint.index as usize != index {
            return Err(format!(
                "skinned player skeleton requires dense joint indices index={index} authored={}",
                joint.index
            ));
        }
        if let Some(parent) = joint.parent_index {
            if parent as usize >= joint_count || parent as usize == index {
                return Err(format!(
                    "skinned player skeleton invalid parent joint={index} parent={parent} joints={joint_count}"
                ));
            }
        }
    }

    let source_to_model = skinned[0].1.source_to_model;
    let source_matrix = Mat4::from_cols_array(&source_to_model);
    let source_values = source_matrix.to_cols_array();
    let inverse_values = source_matrix.inverse().to_cols_array();
    if source_values.iter().any(|value| !value.is_finite())
        || inverse_values.iter().any(|value| !value.is_finite())
    {
        return Err("skinned player source-to-model transform is singular/non-finite".to_owned());
    }

    for (part_index, skin) in skinned {
        if skin.source_to_model != source_to_model {
            return Err(format!(
                "skinned player model source-space transform mismatch part={part_index}"
            ));
        }
        if skin.vertices.is_empty() {
            return Err(format!(
                "skinned player model has empty skin stream part={part_index}"
            ));
        }
        for (vertex_index, vertex) in skin.vertices.iter().enumerate() {
            let mut sum = 0.0_f32;
            let mut positive = 0usize;
            for (&joint, &weight) in vertex
                .joints
                .iter()
                .chain(vertex.joints_extra.iter())
                .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
            {
                if !weight.is_finite() || weight < 0.0 {
                    return Err(format!(
                        "skinned player invalid weight part={part_index} vertex={vertex_index} weight={weight}"
                    ));
                }
                if weight > 0.0 {
                    positive += 1;
                    if joint as usize >= joint_count {
                        return Err(format!(
                            "skinned player joint outside skeleton part={part_index} vertex={vertex_index} joint={joint} joints={joint_count}"
                        ));
                    }
                }
                sum += weight;
            }
            if positive == 0 || !sum.is_finite() || (sum - 1.0).abs() > WEIGHT_EPSILON {
                return Err(format!(
                    "skinned player weights are not normalized part={part_index} vertex={vertex_index} influences={positive} sum={sum}"
                ));
            }
        }
    }
    Ok(Some(source_to_model))
}

pub(super) fn validate_player_palette(
    palette: &[Mat4],
    expected_joints: usize,
    context: &str,
) -> Result<(), String> {
    if palette.len() != expected_joints || palette.is_empty() {
        return Err(format!(
            "player skin palette count mismatch context='{context}' palette={} joints={expected_joints}",
            palette.len()
        ));
    }
    for (joint, matrix) in palette.iter().enumerate() {
        let values = matrix.to_cols_array();
        if values.iter().any(|value| !value.is_finite()) {
            return Err(format!(
                "player skin palette contains non-finite matrix context='{context}' joint={joint}"
            ));
        }
        let max_abs = values
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f32, f32::max);
        if max_abs > 10_000.0 {
            return Err(format!(
                "player skin palette contains unstable transform context='{context}' joint={joint} max_abs={max_abs}"
            ));
        }
        if values[3].abs() > 1.0e-3
            || values[7].abs() > 1.0e-3
            || values[11].abs() > 1.0e-3
            || (values[15] - 1.0).abs() > 1.0e-3
        {
            return Err(format!(
                "player skin palette contains non-affine matrix context='{context}' joint={joint}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::PlayerModelAssignment;

    fn abby_assignment() -> PlayerModelAssignment {
        PlayerModelAssignment {
            enabled: true,
            source: "models/characters/abby/abby.ydd@abby".to_owned(),
            skeleton_source: Some("models/characters/abby/abby.ymt@abby".to_owned()),
            idle_animation: Some("animations/characters/abby/idle.ycd@idle".to_owned()),
            ..PlayerModelAssignment::default()
        }
    }

    #[test]
    fn abby_asset_chain_accepts_only_abby_skeleton_and_clips() {
        validate_player_asset_family(&abby_assignment()).expect("Abby chain");
        let mut mixed = abby_assignment();
        mixed.idle_animation = Some("animations/characters/abigail/idle.ycd@idle".to_owned());
        let error = validate_player_asset_family(&mixed).expect_err("foreign clip must fail");
        assert!(error.contains("Abigail"));
    }

    #[test]
    fn abby_asset_chain_requires_abby_skeleton() {
        let mut missing = abby_assignment();
        missing.skeleton_source = None;
        assert!(validate_player_asset_family(&missing).is_err());
    }
}
