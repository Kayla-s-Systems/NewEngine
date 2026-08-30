use super::*;

pub(super) fn compile_motion<F>(
    graph_name: &str,
    motion: AnimationMotionDefinition,
    parameter_index: &HashMap<String, usize>,
    parameters: &[AnimationGraphParameterDefinition],
    sync_group_index: &HashMap<String, usize>,
    resolve_clip: &mut F,
) -> Result<CompiledAnimationMotion, String>
where
    F: FnMut(&str) -> Result<usize, String>,
{
    match motion {
        AnimationMotionDefinition::Clip(mut motion) => {
            motion.clip_ref = motion.clip_ref.trim().to_owned();
            if motion.clip_ref.is_empty() {
                return Err(format!(
                    "animation graph '{graph_name}' clip motion has an empty reference"
                ));
            }
            let speed = validate_speed(motion.speed, "clip")?;
            Ok(CompiledAnimationMotion::Clip {
                clip_index: resolve_clip(&motion.clip_ref)?,
                speed,
                sync: compile_motion_sync(&motion.sync_group, sync_group_index),
            })
        }
        AnimationMotionDefinition::Blend1D(mut tree) => {
            let parameter_index = resolve_float_graph_parameter(
                graph_name,
                "blend1d",
                &tree.parameter,
                parameter_index,
                parameters,
            )?;
            if tree.samples.is_empty() {
                return Err(format!(
                    "animation graph '{graph_name}' blend1d '{}' contains no samples",
                    tree.parameter
                ));
            }
            let mut samples = Vec::with_capacity(tree.samples.len());
            for sample in tree.samples.drain(..) {
                if !sample.threshold.is_finite() {
                    return Err(format!(
                        "animation graph '{graph_name}' blend1d '{}' has non-finite threshold",
                        tree.parameter
                    ));
                }
                let clip_ref = sample.clip_ref.trim();
                if clip_ref.is_empty() {
                    return Err(format!(
                        "animation graph '{graph_name}' blend1d '{}' has an empty clip reference",
                        tree.parameter
                    ));
                }
                samples.push(CompiledBlendSample1D {
                    threshold: sample.threshold,
                    clip_index: resolve_clip(clip_ref)?,
                    speed: validate_speed(sample.speed, "blend sample")?,
                });
            }
            samples.sort_by(|a, b| a.threshold.total_cmp(&b.threshold));
            for pair in samples.windows(2) {
                if pair[0].threshold == pair[1].threshold {
                    return Err(format!(
                        "animation graph '{graph_name}' blend1d '{}' contains duplicate threshold {}",
                        tree.parameter, pair[0].threshold
                    ));
                }
            }
            if canonical_sync_group(&tree.sync_group).is_some() {
                let reference_speed = samples[0].speed;
                if samples
                    .iter()
                    .any(|sample| (sample.speed - reference_speed).abs() > 1.0e-6)
                {
                    return Err(format!(
                        "animation graph '{graph_name}' synchronized blend1d '{}' requires equal sample speeds",
                        tree.parameter
                    ));
                }
            }
            Ok(CompiledAnimationMotion::Blend1D {
                parameter_index,
                samples,
                sync: compile_motion_sync(&tree.sync_group, sync_group_index),
            })
        }
        AnimationMotionDefinition::Blend2D(mut tree) => {
            let parameter_x_index = resolve_float_graph_parameter(
                graph_name,
                "blend2d x-axis",
                &tree.parameter_x,
                parameter_index,
                parameters,
            )?;
            let parameter_y_index = resolve_float_graph_parameter(
                graph_name,
                "blend2d y-axis",
                &tree.parameter_y,
                parameter_index,
                parameters,
            )?;
            if parameter_x_index == parameter_y_index {
                return Err(format!(
                    "animation graph '{graph_name}' blend2d requires distinct x/y parameters"
                ));
            }
            if tree.samples.is_empty() {
                return Err(format!(
                    "animation graph '{graph_name}' blend2d contains no samples"
                ));
            }
            let mut samples = Vec::with_capacity(tree.samples.len());
            for sample in tree.samples.drain(..) {
                if sample.position.iter().any(|value| !value.is_finite()) {
                    return Err(format!(
                        "animation graph '{graph_name}' blend2d has non-finite sample position"
                    ));
                }
                let clip_ref = sample.clip_ref.trim();
                if clip_ref.is_empty() {
                    return Err(format!(
                        "animation graph '{graph_name}' blend2d has an empty clip reference"
                    ));
                }
                samples.push(CompiledBlendSample2D {
                    position: sample.position,
                    clip_index: resolve_clip(clip_ref)?,
                    speed: validate_speed(sample.speed, "blend2d sample")?,
                });
            }
            let domain = compile_blend2d_domain(graph_name, tree.mode, &samples)?;
            if canonical_sync_group(&tree.sync_group).is_some() {
                let reference_speed = samples[0].speed;
                if samples
                    .iter()
                    .any(|sample| (sample.speed - reference_speed).abs() > 1.0e-6)
                {
                    return Err(format!(
                        "animation graph '{graph_name}' synchronized blend2d requires equal sample speeds"
                    ));
                }
            }
            Ok(CompiledAnimationMotion::Blend2D {
                parameter_x_index,
                parameter_y_index,
                samples,
                domain,
                sync: compile_motion_sync(&tree.sync_group, sync_group_index),
            })
        }
    }
}
