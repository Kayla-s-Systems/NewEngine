fn evaluate_motion(
    context: MotionEvaluationContext<'_>,
    motion: &CompiledAnimationMotion,
    motion_time_seconds: f32,
    runtime: &mut MotionPlaybackRuntime,
    out_pose: &mut Vec<JointLocalPose>,
    scratch: MotionEvaluationScratch<'_>,
) -> Result<MotionEvaluationMeta, String> {
    let graph = context.graph;
    if runtime.cursors.len() != motion.sample_count() {
        return Err("animation graph motion/runtime sample shape mismatch".to_owned());
    }
    match motion {
        CompiledAnimationMotion::Clip { clip_index, .. } => {
            let playback_time = sample_playback_time(graph, motion, 0, motion_time_seconds);
            let compiled_clip = &graph.clips[*clip_index];
            compiled_clip.clip.sample_local_pose_bound(
                playback_time,
                context.skeleton,
                &compiled_clip.binding,
                out_pose,
            )?;
            append_clip_events(
                ClipEventEmission {
                    graph,
                    clip_index: *clip_index,
                    playback_time_seconds: playback_time,
                    emit: context.emit_events,
                    source: context.source,
                    blend_weight: context.source_weight,
                },
                &mut runtime.cursors[0],
                scratch.event_scratch,
                scratch.events,
            )?;
            let mut meta = MotionEvaluationMeta::default();
            meta.add_root_source(0, *clip_index, playback_time, 1.0);
            Ok(meta)
        }
        CompiledAnimationMotion::Blend1D {
            parameter_index,
            samples,
            ..
        } => {
            let value = context
                .parameters
                .get(*parameter_index)
                .and_then(|parameter| parameter.as_float())
                .ok_or_else(|| {
                    "animation graph blend1d parameter/runtime type mismatch".to_owned()
                })?;
            let (left_index, right_index, alpha) = blend_tree_segment(samples, value);
            let left = samples[left_index];
            let right = samples[right_index];
            let dominant_sample = if left_index == right_index || alpha < 0.5 {
                left_index
            } else {
                right_index
            };

            // Keep inactive cursors synchronized to current graph time without synthesizing their
            // historical markers if the blend parameter later enters their segment.
            for sample_index in 0..samples.len() {
                if sample_index == left_index || sample_index == right_index {
                    continue;
                }
                let playback_time =
                    sample_playback_time(graph, motion, sample_index, motion_time_seconds);
                runtime.cursors[sample_index].seek(playback_time)?;
            }

            let left_time = sample_playback_time(graph, motion, left_index, motion_time_seconds);
            let left_clip = &graph.clips[left.clip_index];
            left_clip.clip.sample_local_pose_bound(
                left_time,
                context.skeleton,
                &left_clip.binding,
                scratch.a,
            )?;
            append_clip_events(
                ClipEventEmission {
                    graph,
                    clip_index: left.clip_index,
                    playback_time_seconds: left_time,
                    emit: context.emit_events && dominant_sample == left_index,
                    source: context.source,
                    blend_weight: context.source_weight
                        * if left_index == right_index {
                            1.0
                        } else {
                            1.0 - alpha
                        },
                },
                &mut runtime.cursors[left_index],
                scratch.event_scratch,
                scratch.events,
            )?;

            if left_index == right_index {
                out_pose.clear();
                out_pose.extend_from_slice(scratch.a);
                let mut meta = MotionEvaluationMeta::default();
                meta.add_root_source(left_index, left.clip_index, left_time, 1.0);
                return Ok(meta);
            }

            let right_time = sample_playback_time(graph, motion, right_index, motion_time_seconds);
            let right_clip = &graph.clips[right.clip_index];
            right_clip.clip.sample_local_pose_bound(
                right_time,
                context.skeleton,
                &right_clip.binding,
                scratch.b,
            )?;
            append_clip_events(
                ClipEventEmission {
                    graph,
                    clip_index: right.clip_index,
                    playback_time_seconds: right_time,
                    emit: context.emit_events && dominant_sample == right_index,
                    source: context.source,
                    blend_weight: context.source_weight * alpha,
                },
                &mut runtime.cursors[right_index],
                scratch.event_scratch,
                scratch.events,
            )?;
            blend_pose_arrays(scratch.a, scratch.b, alpha, out_pose)?;
            let mut meta = MotionEvaluationMeta::default();
            meta.add_root_source(left_index, left.clip_index, left_time, 1.0 - alpha);
            meta.add_root_source(right_index, right.clip_index, right_time, alpha);
            Ok(meta)
        }
        CompiledAnimationMotion::Blend2D {
            parameter_x_index,
            parameter_y_index,
            samples,
            domain,
            ..
        } => {
            let x = context
                .parameters
                .get(*parameter_x_index)
                .and_then(|parameter| parameter.as_float())
                .ok_or_else(|| {
                    "animation graph blend2d x parameter/runtime type mismatch".to_owned()
                })?;
            let y = context
                .parameters
                .get(*parameter_y_index)
                .and_then(|parameter| parameter.as_float())
                .ok_or_else(|| {
                    "animation graph blend2d y parameter/runtime type mismatch".to_owned()
                })?;
            let weights = blend2d_weights(samples, domain, x, y);
            let dominant = weights.dominant();

            // Inactive samples track the current synchronized clock without replaying historical
            // markers if the 2D query later enters their region.
            for sample_index in 0..samples.len() {
                if weights.contains(sample_index) {
                    continue;
                }
                let playback_time =
                    sample_playback_time(graph, motion, sample_index, motion_time_seconds);
                runtime.cursors[sample_index].seek(playback_time)?;
            }

            let mut accumulated_weight = 0.0_f32;
            let mut meta = MotionEvaluationMeta::default();
            let mut has_reference_pose = false;
            for entry in &weights.entries[..weights.len] {
                let sample = samples[entry.sample_index];
                let playback_time =
                    sample_playback_time(graph, motion, entry.sample_index, motion_time_seconds);
                let compiled_clip = &graph.clips[sample.clip_index];
                compiled_clip.clip.sample_local_pose_bound(
                    playback_time,
                    context.skeleton,
                    &compiled_clip.binding,
                    scratch.a,
                )?;
                append_clip_events(
                    ClipEventEmission {
                        graph,
                        clip_index: sample.clip_index,
                        playback_time_seconds: playback_time,
                        emit: context.emit_events && entry.sample_index == dominant.sample_index,
                        source: context.source,
                        blend_weight: context.source_weight * entry.weight,
                    },
                    &mut runtime.cursors[entry.sample_index],
                    scratch.event_scratch,
                    scratch.events,
                )?;
                if !has_reference_pose {
                    scratch.b.clone_from(scratch.a);
                    has_reference_pose = true;
                }
                accumulate_weighted_pose_sample(
                    out_pose,
                    scratch.a,
                    scratch.b,
                    context.skeleton.bind_locals(),
                    entry.weight,
                    accumulated_weight <= 1.0e-8,
                )?;
                accumulated_weight += entry.weight;
                meta.add_root_source(
                    entry.sample_index,
                    sample.clip_index,
                    playback_time,
                    entry.weight,
                );
            }
            finish_weighted_pose(out_pose, accumulated_weight)?;
            Ok(meta)
        }
    }
}

fn phase_matched_target_time(
    graph: &CompiledAnimationGraph,
    source_state: usize,
    source_time: f32,
    target_state: usize,
) -> f32 {
    sync_matched_motion_time(
        graph,
        &graph.states[source_state].motion,
        source_time,
        &graph.states[target_state].motion,
    )
    .unwrap_or(0.0)
}

impl AnimationGraphInstance {
    fn begin_transition(
        &mut self,
        graph: &CompiledAnimationGraph,
        transition: ActiveTransitionRuntime,
    ) -> Result<(), String> {
        let source_time = self.states[transition.from_state].time_seconds;
        let target_time = phase_matched_target_time(
            graph,
            transition.from_state,
            source_time,
            transition.to_state,
        );
        self.states[transition.to_state].time_seconds = target_time;
        seek_motion_cursors(
            graph,
            &graph.states[transition.to_state].motion,
            target_time,
            &mut self.states[transition.to_state].motion,
        )?;
        if !transition.source_is_frozen {
            self.frozen_transition_pose.clear();
        }
        self.transition = Some(transition);
        self.root_motion_source = None;
        Ok(())
    }

    fn freeze_last_base_pose(&mut self) -> bool {
        if self.last_base_pose.is_empty() {
            self.frozen_transition_pose.clear();
            false
        } else {
            self.frozen_transition_pose.clone_from(&self.last_base_pose);
            true
        }
    }

    /// Explicit state request used by validated `BlendToState` intents/tools. Authored transition
    /// tables remain the normal automatic path, but callers do not need a GameReady-specific
    /// controller to request a graph state. Explicit requests are authoritative and may interrupt
    /// any active transition; when a previously evaluated pose exists, it becomes the frozen source
    /// of the new blend so the request cannot snap to either endpoint of the interrupted transition.
    pub fn blend_to_state(
        &mut self,
        graph: &CompiledAnimationGraph,
        state: &str,
        blend_seconds: f32,
    ) -> Result<(), String> {
        if !blend_seconds.is_finite() || blend_seconds < 0.0 || blend_seconds > 60.0 {
            return Err(format!(
                "animation graph '{}' explicit blend duration is invalid duration={blend_seconds}",
                graph.name
            ));
        }
        let to_state = graph
            .state_index(state)
            .ok_or_else(|| format!("animation graph '{}' has no state '{state}'", graph.name))?;

        if let Some(active) = self.transition {
            if active.to_state == to_state {
                return Ok(());
            }
            let from_state = active.to_state;
            let source_is_frozen = self.freeze_last_base_pose();
            self.active_state = from_state;
            return self.begin_transition(
                graph,
                ActiveTransitionRuntime {
                    from_state,
                    to_state,
                    elapsed_seconds: 0.0,
                    blend_seconds,
                    source_is_frozen,
                    group_id: None,
                    interruption: AnimationTransitionInterruptionPolicy::Never,
                },
            );
        }

        if self.active_state == to_state {
            return Ok(());
        }
        let from_state = self.active_state;
        self.begin_transition(
            graph,
            ActiveTransitionRuntime {
                from_state,
                to_state,
                elapsed_seconds: 0.0,
                blend_seconds,
                source_is_frozen: false,
                group_id: None,
                interruption: AnimationTransitionInterruptionPolicy::Never,
            },
        )
    }

    pub fn evaluate(
        &mut self,
        graph: &CompiledAnimationGraph,
        skeleton: &AnimationSkeletonRuntime,
        dt_seconds: f32,
        out: &mut AnimationGraphEvaluation,
    ) -> Result<(), String> {
        if self.parameters.len() != graph.parameters.len()
            || self.states.len() != graph.states.len()
            || self.layers.len() != graph.layers.len()
        {
            return Err(format!(
                "animation graph instance does not match compiled graph '{}'",
                graph.name
            ));
        }
        if skeleton.joint_count() == 0 {
            return Err("animation graph evaluation requires a non-empty skeleton".to_owned());
        }
        let dt = if dt_seconds.is_finite() && dt_seconds > 0.0 {
            dt_seconds.min(0.25)
        } else {
            0.0
        };
        out.events.clear();
        out.root_motion = AnimationRootMotionDelta::default();
        out.transition = None;

        // Advance independent layer clocks even when their effective weight is currently zero.
        // Their inactive event cursors are seeked below, preventing historical catch-up.
        for layer in &mut self.layers {
            layer.time_seconds += dt;
        }

        // Active authored transitions may yield to ready transitions from their destination state.
        // The active transition owns the interruption policy; candidate arbitration is already
        // deterministic because each per-state table is priority-descending/authored-order-ascending.
        if let Some(active) = self.transition {
            if active.interruption != AnimationTransitionInterruptionPolicy::Never {
                let source_time = self.states[active.to_state].time_seconds;
                let selected = graph.transitions_by_state[active.to_state]
                    .iter()
                    .find(|candidate| {
                        let group_allowed = match active.interruption {
                            AnimationTransitionInterruptionPolicy::Never => false,
                            AnimationTransitionInterruptionPolicy::SameGroup => {
                                active.group_id.is_some() && active.group_id == candidate.group_id
                            }
                            AnimationTransitionInterruptionPolicy::Any => true,
                        };
                        group_allowed
                            && transition_is_ready(graph, candidate, &self.parameters, source_time)
                    })
                    .cloned();
                if let Some(candidate) = selected {
                    let source_is_frozen = self.freeze_last_base_pose();
                    self.active_state = active.to_state;
                    self.begin_transition(
                        graph,
                        ActiveTransitionRuntime {
                            from_state: candidate.from_state,
                            to_state: candidate.to_state,
                            elapsed_seconds: 0.0,
                            blend_seconds: candidate.blend_seconds,
                            source_is_frozen,
                            group_id: candidate.group_id,
                            interruption: candidate.interruption,
                        },
                    )?;
                }
            }
        }

        if let Some(mut transition) = self.transition {
            if !transition.source_is_frozen {
                let from_speed = graph.states[transition.from_state].speed;
                self.states[transition.from_state].time_seconds += dt * from_speed;
            }
            let to_speed = graph.states[transition.to_state].speed;
            self.states[transition.to_state].time_seconds += dt * to_speed;
            transition.elapsed_seconds += dt;
            self.transition = Some(transition);
        } else {
            let state_index = self.active_state;
            self.states[state_index].time_seconds += dt * graph.states[state_index].speed;
            let source_time = self.states[state_index].time_seconds;
            let selected = graph.transitions_by_state[state_index]
                .iter()
                .find(|transition| {
                    transition_is_ready(graph, transition, &self.parameters, source_time)
                })
                .cloned();
            if let Some(transition) = selected {
                self.begin_transition(
                    graph,
                    ActiveTransitionRuntime {
                        from_state: transition.from_state,
                        to_state: transition.to_state,
                        elapsed_seconds: 0.0,
                        blend_seconds: transition.blend_seconds,
                        source_is_frozen: false,
                        group_id: transition.group_id,
                        interruption: transition.interruption,
                    },
                )?;
            }
        }

        let (root_state_index, root_meta, base_sync_state_index, base_sync_state_time);

        if let Some(transition) = self.transition {
            let alpha = transition_alpha(transition);
            let to_time = self.states[transition.to_state].time_seconds;
            if transition.source_is_frozen {
                if self.frozen_transition_pose.len() != skeleton.joint_count() {
                    return Err(format!(
                        "animation graph '{}' interrupted transition has invalid frozen pose joints={} skeleton={}",
                        graph.name,
                        self.frozen_transition_pose.len(),
                        skeleton.joint_count()
                    ));
                }
                self.scratch_a.clone_from(&self.frozen_transition_pose);
                let to_meta = evaluate_motion(
                    MotionEvaluationContext {
                        graph,
                        skeleton,
                        parameters: &self.parameters,
                        source: AnimationGraphEventSource::State(transition.to_state),
                        emit_events: alpha >= 0.5,
                        source_weight: alpha,
                    },
                    &graph.states[transition.to_state].motion,
                    to_time,
                    &mut self.states[transition.to_state].motion,
                    &mut self.scratch_b,
                    MotionEvaluationScratch {
                        a: &mut self.scratch_layer,
                        b: &mut out.local_pose,
                        event_scratch: &mut self.event_scratch,
                        events: &mut out.events,
                    },
                )?;
                blend_pose_arrays(&self.scratch_a, &self.scratch_b, alpha, &mut out.local_pose)?;
                root_meta = to_meta;
                root_state_index = transition.to_state;
                base_sync_state_index = transition.to_state;
                base_sync_state_time = to_time;
            } else {
                let from_time = self.states[transition.from_state].time_seconds;
                let emit_from = alpha < 0.5;
                let from_meta = evaluate_motion(
                    MotionEvaluationContext {
                        graph,
                        skeleton,
                        parameters: &self.parameters,
                        source: AnimationGraphEventSource::State(transition.from_state),
                        emit_events: emit_from,
                        source_weight: 1.0 - alpha,
                    },
                    &graph.states[transition.from_state].motion,
                    from_time,
                    &mut self.states[transition.from_state].motion,
                    &mut self.scratch_a,
                    MotionEvaluationScratch {
                        a: &mut self.scratch_layer,
                        b: &mut out.local_pose,
                        event_scratch: &mut self.event_scratch,
                        events: &mut out.events,
                    },
                )?;
                let to_meta = evaluate_motion(
                    MotionEvaluationContext {
                        graph,
                        skeleton,
                        parameters: &self.parameters,
                        source: AnimationGraphEventSource::State(transition.to_state),
                        emit_events: !emit_from,
                        source_weight: alpha,
                    },
                    &graph.states[transition.to_state].motion,
                    to_time,
                    &mut self.states[transition.to_state].motion,
                    &mut self.scratch_b,
                    MotionEvaluationScratch {
                        a: &mut self.scratch_layer,
                        b: &mut out.local_pose,
                        event_scratch: &mut self.event_scratch,
                        events: &mut out.events,
                    },
                )?;
                blend_pose_arrays(&self.scratch_a, &self.scratch_b, alpha, &mut out.local_pose)?;
                root_meta = if emit_from { from_meta } else { to_meta };
                root_state_index = if emit_from {
                    transition.from_state
                } else {
                    transition.to_state
                };
                let root_state_time = self.states[root_state_index].time_seconds;
                base_sync_state_index = root_state_index;
                base_sync_state_time = root_state_time;
            }
            out.transition = Some(AnimationGraphTransitionSnapshot {
                from_state: transition.from_state,
                to_state: transition.to_state,
                alpha,
            });
            if alpha >= 1.0 - 1.0e-6 {
                self.active_state = transition.to_state;
                self.transition = None;
                self.frozen_transition_pose.clear();
            }
        } else {
            let state_index = self.active_state;
            let state_time = self.states[state_index].time_seconds;
            root_meta = evaluate_motion(
                MotionEvaluationContext {
                    graph,
                    skeleton,
                    parameters: &self.parameters,
                    source: AnimationGraphEventSource::State(state_index),
                    emit_events: true,
                    source_weight: 1.0,
                },
                &graph.states[state_index].motion,
                state_time,
                &mut self.states[state_index].motion,
                &mut out.local_pose,
                MotionEvaluationScratch {
                    a: &mut self.scratch_a,
                    b: &mut self.scratch_b,
                    event_scratch: &mut self.event_scratch,
                    events: &mut out.events,
                },
            )?;
            base_sync_state_index = state_index;
            base_sync_state_time = state_time;
            root_state_index = state_index;
        }

        let root_mode = graph.states[root_state_index].root_motion;
        if root_mode != AnimationRootMotionMode::Disabled {
            if let Some(root_joint) = graph.root_motion_joint_index {
                out.root_motion = self.extract_root_motion(
                    graph,
                    skeleton,
                    root_joint,
                    root_state_index,
                    self.states[root_state_index].time_seconds,
                    root_meta,
                )?;
            }
        } else {
            self.root_motion_source = None;
        }

        // Interruption snapshots own only the base state-machine pose. Graph layers are evaluated
        // again every frame, so caching the post-layer result here would apply them twice after an
        // interruption. Root-motion extraction is normalized in the cached source as well.
        self.last_base_pose.clone_from(&out.local_pose);
        if root_mode == AnimationRootMotionMode::ExtractAndRemove {
            if let Some(root_joint) = graph.root_motion_joint_index {
                if root_joint < self.last_base_pose.len() {
                    self.last_base_pose[root_joint] = skeleton.bind_locals()[root_joint];
                }
            }
        }

        // Layer stack is evaluated after the base state machine. Matching sync groups borrow the
        // authoritative unwrapped phase from the current event/root leader state.
        for layer_index in 0..graph.layers.len() {
            let layer = &graph.layers[layer_index];
            let weight = layer_effective_weight(layer, &self.parameters);
            let synchronized_time = sync_matched_motion_time(
                graph,
                &graph.states[base_sync_state_index].motion,
                base_sync_state_time,
                &layer.motion,
            );
            let layer_time = synchronized_time.unwrap_or(self.layers[layer_index].time_seconds);
            if weight <= 1.0e-6 {
                seek_motion_cursors(
                    graph,
                    &layer.motion,
                    layer_time,
                    &mut self.layers[layer_index].motion,
                )?;
                continue;
            }
            let emit_events = weight + 1.0e-6 >= layer.event_weight_threshold;
            let _meta = evaluate_motion(
                MotionEvaluationContext {
                    graph,
                    skeleton,
                    parameters: &self.parameters,
                    source: AnimationGraphEventSource::Layer(layer_index),
                    emit_events,
                    source_weight: weight,
                },
                &layer.motion,
                layer_time,
                &mut self.layers[layer_index].motion,
                &mut self.scratch_layer,
                MotionEvaluationScratch {
                    a: &mut self.scratch_a,
                    b: &mut self.scratch_b,
                    event_scratch: &mut self.event_scratch,
                    events: &mut out.events,
                },
            )?;
            match layer.mode {
                AnimationLayerBlendMode::Override => apply_override_layer(
                    &mut out.local_pose,
                    &self.scratch_layer,
                    &layer.mask,
                    weight,
                )?,
                AnimationLayerBlendMode::Additive => apply_additive_layer(
                    &mut out.local_pose,
                    &self.scratch_layer,
                    skeleton.bind_locals(),
                    &layer.mask,
                    weight,
                )?,
            }
        }

        if root_mode == AnimationRootMotionMode::ExtractAndRemove {
            if let Some(root_joint) = graph.root_motion_joint_index {
                if root_joint < out.local_pose.len() {
                    out.local_pose[root_joint] = skeleton.bind_locals()[root_joint];
                }
            }
        }

        out.active_state = self.active_state;
        if out.local_pose.len() != skeleton.joint_count() {
            return Err(format!(
                "animation graph '{}' produced incomplete pose joints={} skeleton={}",
                graph.name,
                out.local_pose.len(),
                skeleton.joint_count()
            ));
        }
        Ok(())
    }

    fn extract_root_motion(
        &mut self,
        graph: &CompiledAnimationGraph,
        skeleton: &AnimationSkeletonRuntime,
        root_joint: usize,
        state_index: usize,
        motion_time_seconds: f32,
        meta: MotionEvaluationMeta,
    ) -> Result<AnimationRootMotionDelta, String> {
        let current_source = RootMotionRuntimeSource {
            state_index,
            motion_time_seconds,
        };
        let Some(previous) = self.root_motion_source.replace(current_source) else {
            return Ok(AnimationRootMotionDelta::default());
        };
        if previous.state_index != state_index || motion_time_seconds < previous.motion_time_seconds
        {
            return Ok(AnimationRootMotionDelta::default());
        }
        if meta.root_source_count == 0 {
            return Ok(AnimationRootMotionDelta::default());
        }
        let motion = &graph.states[state_index].motion;
        let mut translation = [0.0_f32; 3];
        let mut rotation_acc = [0.0_f32; 4];
        let mut total_weight = 0.0_f32;
        for source in &meta.root_sources[..meta.root_source_count] {
            if source.sample_index >= motion.sample_count() {
                return Err(format!(
                    "animation root-motion sample index outside motion samples state={} sample={} samples={}",
                    state_index,
                    source.sample_index,
                    motion.sample_count()
                ));
            }
            let previous_playback = sample_playback_time(
                graph,
                motion,
                source.sample_index,
                previous.motion_time_seconds,
            );
            if source.playback_time_seconds + 1.0e-6 < previous_playback {
                continue;
            }
            let compiled_clip = &graph.clips[source.clip_index];
            let delta = root_motion_delta_between(
                &compiled_clip.clip,
                &compiled_clip.binding,
                skeleton,
                root_joint,
                previous_playback,
                source.playback_time_seconds,
            )?;
            let weight = source.weight.max(0.0);
            for (component, value) in translation.iter_mut().zip(delta.translation) {
                *component += value * weight;
            }
            let mut rotation = delta.rotation;
            if rotation[3] < 0.0 {
                for component in &mut rotation {
                    *component = -*component;
                }
            }
            for (component, value) in rotation_acc.iter_mut().zip(rotation) {
                *component += value * weight;
            }
            total_weight += weight;
        }
        if total_weight <= 1.0e-8 {
            return Ok(AnimationRootMotionDelta::default());
        }
        let rotation = Quat::from_xyzw(
            rotation_acc[0],
            rotation_acc[1],
            rotation_acc[2],
            rotation_acc[3],
        )
        .normalize_or_identity();
        Ok(AnimationRootMotionDelta {
            translation,
            rotation: quat_array(rotation),
            valid: true,
        })
    }
}
