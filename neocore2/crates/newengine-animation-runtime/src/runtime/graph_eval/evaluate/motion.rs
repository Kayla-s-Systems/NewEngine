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
            for (sample_index, cursor) in runtime.cursors.iter_mut().enumerate() {
                if sample_index == left_index || sample_index == right_index {
                    continue;
                }
                let playback_time =
                    sample_playback_time(graph, motion, sample_index, motion_time_seconds);
                cursor.seek(playback_time)?;
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
            for (sample_index, cursor) in runtime.cursors.iter_mut().enumerate() {
                if weights.contains(sample_index) {
                    continue;
                }
                let playback_time =
                    sample_playback_time(graph, motion, sample_index, motion_time_seconds);
                cursor.seek(playback_time)?;
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
