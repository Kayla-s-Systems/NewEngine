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
}
