impl AnimationGraphInstance {
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
}
