#[inline]
fn canonical_graph_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_speed(value: f32, label: &str) -> Result<f32, String> {
    if !value.is_finite() || value <= 0.0 || value > 100.0 {
        return Err(format!(
            "animation graph {label} speed must be finite in (0,100] speed={value}"
        ));
    }
    Ok(value)
}

fn resolve_float_graph_parameter(
    graph_name: &str,
    node_label: &str,
    parameter_name: &str,
    parameter_index: &HashMap<String, usize>,
    parameters: &[AnimationGraphParameterDefinition],
) -> Result<usize, String> {
    let index = parameter_index
        .get(&canonical_graph_key(parameter_name))
        .copied()
        .ok_or_else(|| {
            format!(
                "animation graph '{graph_name}' {node_label} references unknown parameter '{parameter_name}'"
            )
        })?;
    if !matches!(
        parameters[index].default,
        AnimationGraphParameterValue::Float(_)
    ) {
        return Err(format!(
            "animation graph '{graph_name}' {node_label} parameter '{parameter_name}' is not float"
        ));
    }
    Ok(index)
}

fn canonical_sync_group(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn compile_motion_sync(
    value: &Option<String>,
    sync_group_index: &HashMap<String, usize>,
) -> Option<CompiledMotionSync> {
    canonical_sync_group(value).map(|key| CompiledMotionSync {
        marker_group_index: sync_group_index.get(&key).copied(),
        key,
    })
}

#[path = "graph_compile/blend2d.rs"]
mod graph_compile_blend2d;
use graph_compile_blend2d::{
    blend2d_position_distance_squared, compile_blend2d_domain, normalize_angle_radians,
};
#[path = "graph_compile/motion.rs"]
mod graph_compile_motion;
use graph_compile_motion::compile_motion;

fn compile_sync_marker_track(
    graph_name: &str,
    group: &CompiledAnimationSyncGroup,
    clip: &AnimationClip,
) -> Result<CompiledSyncMarkerTrack, String> {
    if !clip.looped {
        return Err(format!(
            "animation graph '{graph_name}' marker sync group '{}' requires looped clip '{}'",
            group.name, clip.name
        ));
    }
    let mut markers = Vec::with_capacity(group.marker_tags.len());
    let mut counts = vec![0usize; group.marker_tags.len()];
    for event in &clip.events {
        let key = canonical_graph_key(&event.tag);
        let Some(marker_index) = group.marker_index.get(&key).copied() else {
            continue;
        };
        counts[marker_index] += 1;
        markers.push(CompiledSyncMarker {
            marker_index,
            time_seconds: event.time_seconds,
        });
    }
    for (marker_index, count) in counts.into_iter().enumerate() {
        if count != 1 {
            return Err(format!(
                "animation graph '{graph_name}' marker sync group '{}' clip '{}' requires exactly one '{}' marker per cycle found={count}",
                group.name, clip.name, group.marker_tags[marker_index]
            ));
        }
    }
    markers.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    for pair in markers.windows(2) {
        if pair[0].time_seconds + 1.0e-6 >= pair[1].time_seconds {
            return Err(format!(
                "animation graph '{graph_name}' marker sync group '{}' clip '{}' contains coincident marker times",
                group.name, clip.name
            ));
        }
    }
    let first = markers[0].marker_index;
    for (offset, marker) in markers.iter().enumerate() {
        let expected = (first + offset) % group.marker_tags.len();
        if marker.marker_index != expected {
            return Err(format!(
                "animation graph '{graph_name}' marker sync group '{}' clip '{}' marker order is not a cyclic rotation of authored vocabulary",
                group.name, clip.name
            ));
        }
    }
    Ok(CompiledSyncMarkerTrack { markers })
}

fn register_motion_sync_tracks(
    graph_name: &str,
    motion: &CompiledAnimationMotion,
    clips: &[CompiledAnimationGraphClip],
    sync_groups: &[CompiledAnimationSyncGroup],
    tracks: &mut [Vec<Option<CompiledSyncMarkerTrack>>],
) -> Result<(), String> {
    let Some(group_index) = motion.marker_sync_group_index() else {
        return Ok(());
    };
    let group = &sync_groups[group_index];
    for clip_index in motion.clip_indices() {
        if tracks[group_index][clip_index].is_none() {
            tracks[group_index][clip_index] = Some(compile_sync_marker_track(
                graph_name,
                group,
                &clips[clip_index].clip,
            )?);
        }
    }
    Ok(())
}

impl CompiledAnimationGraph {
    pub fn compile<F>(
        definition: AnimationGraphDefinition,
        skeleton: &AnimationSkeletonRuntime,
        mut load_clip: F,
    ) -> Result<Self, String>
    where
        F: FnMut(&str) -> Result<Arc<AnimationClip>, String>,
    {
        let name = definition.name.trim().to_owned();
        if name.is_empty() {
            return Err("animation graph name is empty".to_owned());
        }
        if definition.states.is_empty() {
            return Err(format!("animation graph '{name}' contains no states"));
        }

        let mut parameter_index = HashMap::with_capacity(definition.parameters.len());
        let mut parameters = Vec::with_capacity(definition.parameters.len());
        for mut parameter in definition.parameters {
            parameter.name = parameter.name.trim().to_owned();
            if parameter.name.is_empty() {
                return Err(format!(
                    "animation graph '{name}' contains an empty parameter name"
                ));
            }
            if let AnimationGraphParameterValue::Float(value) = parameter.default {
                if !value.is_finite() {
                    return Err(format!(
                        "animation graph '{name}' parameter '{}' has non-finite default",
                        parameter.name
                    ));
                }
            }
            let key = canonical_graph_key(&parameter.name);
            if parameter_index.insert(key, parameters.len()).is_some() {
                return Err(format!(
                    "animation graph '{name}' contains duplicate parameter '{}'",
                    parameter.name
                ));
            }
            parameters.push(parameter);
        }

        let mut state_index = HashMap::with_capacity(definition.states.len());
        for (index, state) in definition.states.iter().enumerate() {
            let state_name = state.name.trim();
            if state_name.is_empty() {
                return Err(format!(
                    "animation graph '{name}' contains an empty state name"
                ));
            }
            let key = canonical_graph_key(state_name);
            if state_index.insert(key, index).is_some() {
                return Err(format!(
                    "animation graph '{name}' contains duplicate state '{state_name}'"
                ));
            }
        }
        let entry_state = state_index
            .get(&canonical_graph_key(&definition.entry_state))
            .copied()
            .ok_or_else(|| {
                format!(
                    "animation graph '{name}' entry state '{}' is not declared",
                    definition.entry_state
                )
            })?;

        let mut sync_groups = Vec::with_capacity(definition.sync_groups.len());
        let mut sync_group_index = HashMap::with_capacity(definition.sync_groups.len());
        for group in &definition.sync_groups {
            let group_name = group.name.trim();
            if group_name.is_empty() {
                return Err(format!(
                    "animation graph '{name}' contains an empty sync group name"
                ));
            }
            let key = canonical_graph_key(group_name);
            if sync_group_index.contains_key(&key) {
                return Err(format!(
                    "animation graph '{name}' contains duplicate sync group '{group_name}'"
                ));
            }
            if group.markers.len() < 2 {
                return Err(format!(
                    "animation graph '{name}' sync group '{group_name}' requires at least two markers"
                ));
            }
            let mut marker_tags = Vec::with_capacity(group.markers.len());
            let mut marker_index = HashMap::with_capacity(group.markers.len());
            for marker in &group.markers {
                let marker = marker.trim();
                if marker.is_empty() {
                    return Err(format!(
                        "animation graph '{name}' sync group '{group_name}' contains an empty marker tag"
                    ));
                }
                let marker_key = canonical_graph_key(marker);
                if marker_index
                    .insert(marker_key.clone(), marker_tags.len())
                    .is_some()
                {
                    return Err(format!(
                        "animation graph '{name}' sync group '{group_name}' contains duplicate marker '{marker}'"
                    ));
                }
                marker_tags.push(marker_key);
            }
            let index = sync_groups.len();
            sync_group_index.insert(key, index);
            sync_groups.push(CompiledAnimationSyncGroup {
                name: group_name.to_owned(),
                marker_tags,
                marker_index,
            });
        }

        let mut clips = Vec::<CompiledAnimationGraphClip>::new();
        let mut clip_index = HashMap::<String, usize>::new();
        let mut resolve_clip = |reference: &str| -> Result<usize, String> {
            let parsed = AnimationClipReference::parse(reference)?;
            let key = parsed.canonical_clip_key;
            if let Some(index) = clip_index.get(&key).copied() {
                return Ok(index);
            }
            let clip = load_clip(reference).map_err(|error| {
                format!("animation graph '{name}' clip load failed ref='{reference}': {error}")
            })?;
            clip.validate_structure()?;
            let binding = skeleton.bind_clip(&clip)?;
            let index = clips.len();
            clips.push(CompiledAnimationGraphClip {
                reference: reference.trim().replace('\\', "/"),
                clip,
                binding,
            });
            clip_index.insert(key, index);
            Ok(index)
        };

        let mut states = Vec::with_capacity(definition.states.len());
        for state in definition.states {
            states.push(CompiledAnimationGraphState {
                name: state.name.trim().to_owned(),
                motion: compile_motion(
                    &name,
                    state.motion,
                    &parameter_index,
                    &parameters,
                    &sync_group_index,
                    &mut resolve_clip,
                )?,
                speed: validate_speed(state.speed, "state")?,
                root_motion: state.root_motion,
            });
        }

        let mut transitions_by_state = vec![Vec::new(); states.len()];
        let mut transition_group_index = HashMap::<String, usize>::new();
        for (authored_order, transition) in definition.transitions.into_iter().enumerate() {
            let from_state = state_index
                .get(&canonical_graph_key(&transition.from))
                .copied()
                .ok_or_else(|| {
                    format!(
                        "animation graph '{name}' transition references unknown source state '{}'",
                        transition.from
                    )
                })?;
            let to_state = state_index
                .get(&canonical_graph_key(&transition.to))
                .copied()
                .ok_or_else(|| {
                    format!(
                        "animation graph '{name}' transition references unknown target state '{}'",
                        transition.to
                    )
                })?;
            if from_state == to_state {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' is self-referential",
                    transition.from, transition.to
                ));
            }
            if !transition.blend_seconds.is_finite()
                || transition.blend_seconds < 0.0
                || transition.blend_seconds > 60.0
            {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' has invalid blend duration {}",
                    transition.from, transition.to, transition.blend_seconds
                ));
            }
            if transition
                .exit_time_normalized
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' has invalid normalized exit time",
                    transition.from, transition.to
                ));
            }
            let group_id = match transition.group.as_deref() {
                None => None,
                Some(group) => {
                    let group = group.trim();
                    if group.is_empty() {
                        return Err(format!(
                            "animation graph '{name}' transition '{}' -> '{}' has an empty interruption group",
                            transition.from, transition.to
                        ));
                    }
                    let key = canonical_graph_key(group);
                    let next = transition_group_index.len();
                    Some(*transition_group_index.entry(key).or_insert(next))
                }
            };
            if transition.interruption == AnimationTransitionInterruptionPolicy::SameGroup
                && group_id.is_none()
            {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' uses same_group interruption without a group",
                    transition.from, transition.to
                ));
            }
            let mut conditions = Vec::with_capacity(transition.conditions.len());
            for condition in transition.conditions {
                let compiled = match condition {
                    AnimationTransitionCondition::Bool { parameter, equals } => {
                        let index = parameter_index
                            .get(&canonical_graph_key(&parameter))
                            .copied()
                            .ok_or_else(|| {
                                format!(
                                    "animation graph '{name}' transition references unknown bool parameter '{parameter}'"
                                )
                            })?;
                        if !matches!(
                            parameters[index].default,
                            AnimationGraphParameterValue::Bool(_)
                        ) {
                            return Err(format!(
                                "animation graph '{name}' transition expects bool parameter '{parameter}'"
                            ));
                        }
                        CompiledTransitionCondition::Bool {
                            parameter_index: index,
                            equals,
                        }
                    }
                    AnimationTransitionCondition::Float {
                        parameter,
                        comparison,
                        value,
                    } => {
                        if !value.is_finite() {
                            return Err(format!(
                                "animation graph '{name}' transition float threshold is non-finite parameter='{parameter}'"
                            ));
                        }
                        let index = parameter_index
                            .get(&canonical_graph_key(&parameter))
                            .copied()
                            .ok_or_else(|| {
                                format!(
                                    "animation graph '{name}' transition references unknown float parameter '{parameter}'"
                                )
                            })?;
                        if !matches!(
                            parameters[index].default,
                            AnimationGraphParameterValue::Float(_)
                        ) {
                            return Err(format!(
                                "animation graph '{name}' transition expects float parameter '{parameter}'"
                            ));
                        }
                        CompiledTransitionCondition::Float {
                            parameter_index: index,
                            comparison,
                            value,
                        }
                    }
                };
                conditions.push(compiled);
            }
            transitions_by_state[from_state].push(CompiledAnimationGraphTransition {
                from_state,
                to_state,
                conditions,
                exit_time_normalized: transition.exit_time_normalized,
                blend_seconds: transition.blend_seconds,
                priority: transition.priority,
                authored_order,
                group_id,
                interruption: transition.interruption,
            });
        }
        for transitions in &mut transitions_by_state {
            transitions.sort_by(|a, b| {
                b.priority
                    .cmp(&a.priority)
                    .then_with(|| a.authored_order.cmp(&b.authored_order))
            });
        }

        let mut layers = Vec::with_capacity(definition.layers.len());
        let mut layer_names = HashSet::new();
        for layer in definition.layers {
            let layer_name = layer.name.trim().to_owned();
            if layer_name.is_empty() {
                return Err(format!(
                    "animation graph '{name}' contains an empty layer name"
                ));
            }
            if !layer_names.insert(canonical_graph_key(&layer_name)) {
                return Err(format!(
                    "animation graph '{name}' contains duplicate layer '{layer_name}'"
                ));
            }
            if !layer.weight.is_finite() || !(0.0..=1.0).contains(&layer.weight) {
                return Err(format!(
                    "animation graph '{name}' layer '{layer_name}' weight must be in [0,1]"
                ));
            }
            if !layer.event_weight_threshold.is_finite()
                || !(0.0..=1.0).contains(&layer.event_weight_threshold)
            {
                return Err(format!(
                    "animation graph '{name}' layer '{layer_name}' event threshold must be in [0,1]"
                ));
            }
            let weight_parameter_index = layer
                .weight_parameter
                .as_deref()
                .map(|parameter| {
                    let index = parameter_index
                        .get(&canonical_graph_key(parameter))
                        .copied()
                        .ok_or_else(|| {
                            format!(
                                "animation graph '{name}' layer '{layer_name}' references unknown weight parameter '{parameter}'"
                            )
                        })?;
                    if !matches!(parameters[index].default, AnimationGraphParameterValue::Float(_)) {
                        return Err(format!(
                            "animation graph '{name}' layer '{layer_name}' weight parameter '{parameter}' is not float"
                        ));
                    }
                    Ok(index)
                })
                .transpose()?;
            let mask = compile_bone_mask(&name, &layer_name, layer.mask, skeleton)?;
            layers.push(CompiledAnimationGraphLayer {
                name: layer_name,
                motion: compile_motion(
                    &name,
                    layer.motion,
                    &parameter_index,
                    &parameters,
                    &sync_group_index,
                    &mut resolve_clip,
                )?,
                mode: layer.mode,
                weight: layer.weight,
                weight_parameter_index,
                mask,
                event_weight_threshold: layer.event_weight_threshold,
            });
        }

        let mut sync_marker_tracks = vec![vec![None; clips.len()]; sync_groups.len()];
        for state in &states {
            register_motion_sync_tracks(
                &name,
                &state.motion,
                &clips,
                &sync_groups,
                &mut sync_marker_tracks,
            )?;
        }
        for layer in &layers {
            register_motion_sync_tracks(
                &name,
                &layer.motion,
                &clips,
                &sync_groups,
                &mut sync_marker_tracks,
            )?;
        }

        let needs_root_motion = states
            .iter()
            .any(|state| state.root_motion != AnimationRootMotionMode::Disabled);
        let root_motion_joint_index = if needs_root_motion {
            let tag = definition.root_motion_joint_tag.ok_or_else(|| {
                format!(
                    "animation graph '{name}' extracts root motion but root_motion_joint_tag is absent"
                )
            })?;
            Some(skeleton.resolve_joint_tag(tag).map_err(|error| {
                format!("animation graph '{name}' root-motion joint invalid: {error}")
            })?)
        } else {
            definition
                .root_motion_joint_tag
                .map(|tag| skeleton.resolve_joint_tag(tag))
                .transpose()?
        };

        Ok(Self {
            name,
            entry_state,
            parameters,
            parameter_index,
            states,
            state_index,
            transitions_by_state,
            layers,
            clips,
            sync_marker_tracks,
            root_motion_joint_index,
        })
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn entry_state_index(&self) -> usize {
        self.entry_state
    }

    #[inline]
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    #[inline]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    #[inline]
    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }

    #[inline]
    pub fn state_name(&self, index: usize) -> Option<&str> {
        self.states.get(index).map(|state| state.name.as_str())
    }

    #[inline]
    pub fn state_index(&self, name: &str) -> Option<usize> {
        self.state_index.get(&canonical_graph_key(name)).copied()
    }

    #[inline]
    pub fn layer_name(&self, index: usize) -> Option<&str> {
        self.layers.get(index).map(|layer| layer.name.as_str())
    }

    #[inline]
    pub fn clip_reference(&self, index: usize) -> Option<&str> {
        self.clips.get(index).map(|clip| clip.reference.as_str())
    }

    #[inline]
    pub fn clip(&self, index: usize) -> Option<&Arc<AnimationClip>> {
        self.clips.get(index).map(|clip| &clip.clip)
    }

    #[inline]
    pub fn parameter_index(&self, name: &str) -> Option<usize> {
        self.parameter_index
            .get(&canonical_graph_key(name))
            .copied()
    }
}

fn compile_bone_mask(
    graph_name: &str,
    layer_name: &str,
    definition: Option<AnimationBoneMaskDefinition>,
    skeleton: &AnimationSkeletonRuntime,
) -> Result<Vec<f32>, String> {
    let Some(definition) = definition else {
        return Ok(vec![1.0; skeleton.joint_count()]);
    };
    let mut mask = vec![0.0_f32; skeleton.joint_count()];
    for root in definition.roots {
        if !root.weight.is_finite() || !(0.0..=1.0).contains(&root.weight) {
            return Err(format!(
                "animation graph '{graph_name}' layer '{layer_name}' mask weight is invalid tag={} weight={}",
                root.joint_tag, root.weight
            ));
        }
        let joint = skeleton
            .resolve_joint_tag(root.joint_tag)
            .map_err(|error| {
                format!(
                "animation graph '{graph_name}' layer '{layer_name}' mask joint invalid: {error}"
            )
            })?;
        mask[joint] = root.weight;
        if root.include_descendants {
            for (candidate, candidate_weight) in mask.iter_mut().enumerate() {
                let mut parent = skeleton.parent_indices[candidate];
                while let Some(parent_index) = parent {
                    if parent_index == joint {
                        *candidate_weight = root.weight;
                        break;
                    }
                    parent = skeleton.parent_indices[parent_index];
                }
            }
        }
    }
    Ok(mask)
}
