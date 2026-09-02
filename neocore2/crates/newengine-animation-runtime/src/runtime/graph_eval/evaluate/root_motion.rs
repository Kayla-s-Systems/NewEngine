impl AnimationGraphInstance {
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
