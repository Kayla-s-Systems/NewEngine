#[derive(Clone, Copy, Debug)]
struct ResolvedJointCopyRule {
    source_index: usize,
    target_index: usize,
    channels: newengine_engine_runtime::gameplay::PlayerJointChannels,
}

fn resolve_helper_pose_copy_rules(
    skeleton: &ModelSkeletonMetadata,
    authored: &[newengine_engine_runtime::gameplay::PlayerJointCopyRule],
) -> Result<Vec<ResolvedJointCopyRule>, String> {
    let mut resolved = Vec::with_capacity(authored.len());
    for rule in authored {
        let source_name = rule.source_joint.trim();
        let target_name = rule.target_joint.trim();
        if source_name.is_empty()
            || target_name.is_empty()
            || source_name.eq_ignore_ascii_case(target_name)
            || !rule.channels.any()
        {
            return Err(format!(
                "invalid authored joint-copy rule source='{}' target='{}' channels={:?}",
                rule.source_joint, rule.target_joint, rule.channels
            ));
        }
        let source_index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == source_name)
            .ok_or_else(|| format!("authored joint-copy source is absent name='{source_name}'"))?;
        let target_index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == target_name)
            .ok_or_else(|| format!("authored joint-copy target is absent name='{target_name}'"))?;
        resolved.push(ResolvedJointCopyRule {
            source_index,
            target_index,
            channels: rule.channels,
        });
    }
    Ok(resolved)
}

#[inline]
fn synchronize_helper_pose(rules: &[ResolvedJointCopyRule], pose: &mut [JointLocalPose]) {
    for rule in rules {
        if rule.source_index >= pose.len() || rule.target_index >= pose.len() {
            continue;
        }
        let source = pose[rule.source_index];
        let target = &mut pose[rule.target_index];
        if rule.channels.translation {
            target.translation = source.translation;
        }
        if rule.channels.rotation {
            target.rotation = source.rotation;
        }
        if rule.channels.scale {
            target.scale = source.scale;
        }
    }
}
