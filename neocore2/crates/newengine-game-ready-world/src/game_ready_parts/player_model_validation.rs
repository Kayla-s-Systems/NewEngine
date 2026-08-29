use super::*;

use newengine_math::Mat4;
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[inline]
fn normalized_ref(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn character_asset_owner(value: &str) -> Option<String> {
    let normalized = normalized_ref(value);
    let path = normalized.split('@').next().unwrap_or(&normalized);
    let parts = path.split('/').collect::<Vec<_>>();
    parts.windows(2).find_map(|pair| {
        (pair[0] == "characters" && !pair[1].is_empty()).then(|| pair[1].to_owned())
    })
}

pub(super) fn validate_player_asset_family(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
) -> Result<(), String> {
    let Some(model_owner) = character_asset_owner(&assignment.source) else {
        return Ok(());
    };
    let skeleton = assignment.skeleton_source.as_deref().unwrap_or_default();
    if skeleton.trim().is_empty() {
        return Err(format!(
            "character asset chain requires authored skeleton owner='{model_owner}' model='{}'",
            assignment.source
        ));
    }

    for (role, reference) in [
        ("skeleton", Some(skeleton)),
        ("idle_animation", assignment.idle_animation.as_deref()),
        ("walk_animation", assignment.walk_animation.as_deref()),
        ("run_animation", assignment.run_animation.as_deref()),
        ("sprint_animation", assignment.sprint_animation.as_deref()),
        (
            "crouch_idle_animation",
            assignment.crouch_idle_animation.as_deref(),
        ),
        (
            "crouch_walk_animation",
            assignment.crouch_walk_animation.as_deref(),
        ),
        ("jump_animation", assignment.jump_animation.as_deref()),
        ("fall_animation", assignment.fall_animation.as_deref()),
        (
            "unarmed_ready_animation",
            assignment.presentation.unarmed_ready_animation.as_deref(),
        ),
        (
            "unarmed_attack_animation",
            assignment.presentation.unarmed_attack_animation.as_deref(),
        ),
    ] {
        let Some(reference) = reference else { continue };
        let Some(reference_owner) = character_asset_owner(reference) else {
            return Err(format!(
                "character asset reference has no characters/<owner> namespace model_owner='{model_owner}' role='{role}' ref='{reference}'"
            ));
        };
        if reference_owner != model_owner {
            return Err(format!(
                "character asset ownership mismatch model_owner='{model_owner}' reference_owner='{reference_owner}' role='{role}' ref='{reference}'"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_player_skin_contract(
    _assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
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
                    let joint = joint as usize;
                    if joint >= joint_count {
                        return Err(format!(
                            "skinned player joint outside authored skeleton part={part_index} vertex={vertex_index} joint={joint} skeleton_joints={joint_count}; supplemental game-specific palettes are not a runtime fallback"
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

    fn owner_a_assignment() -> PlayerModelAssignment {
        PlayerModelAssignment {
            enabled: true,
            source: "models/characters/owner_a/body.ydd@body".to_owned(),
            skeleton_source: Some("models/characters/owner_a/body.ymt@body".to_owned()),
            idle_animation: Some("animations/characters/owner_a/locomotion.ycd@idle".to_owned()),
            ..PlayerModelAssignment::default()
        }
    }

    #[test]
    fn character_asset_chain_rejects_cross_owner_skeleton_or_clips() {
        validate_player_asset_family(&owner_a_assignment()).expect("owned chain");
        let mut mixed = owner_a_assignment();
        mixed.idle_animation = Some("animations/characters/owner_b/locomotion.ycd@idle".to_owned());
        let error = validate_player_asset_family(&mixed).expect_err("foreign clip must fail");
        assert!(error.contains("ownership mismatch"), "{error}");
    }

    #[test]
    fn namespaced_character_asset_chain_requires_skeleton() {
        let mut missing = owner_a_assignment();
        missing.skeleton_source = None;
        assert!(validate_player_asset_family(&missing).is_err());
    }
}
