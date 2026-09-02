impl Module<()> for AudioOrchestrationRuntimeModule {
    fn id(&self) -> &'static str {
        "engine.audio.orchestration.runtime"
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        ctx.resources_mut().insert(self.handle.clone());
        ctx.resources_mut()
            .insert(AudioTransportHandle::new(self.handle.clone()));
        ctx.resources_mut()
            .insert(InteractiveMusicHandle::new(self.handle.clone()));
        self.publish_runtime_state(ctx);
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        self.refresh_provider_clock();
        self.process_commands();
        if self.provider_clock.is_none() && self.transport.has_pending_actions() {
            self.refresh_provider_clock();
        }
        self.prearm_pending_transport_actions();
        let dt = ctx.frame().map(|frame| frame.dt).unwrap_or(1.0 / 60.0);
        let (markers, due_actions) = self.advance_transport_clock(dt);
        self.publish_transport_markers(markers);
        for due in due_actions {
            self.advance_snapshots_to_sample(due.intended_sample);
            self.advance_scalar_transitions_to_sample(due.intended_sample);
            self.advance_instance_gain_transitions_to_sample(due.intended_sample);
            self.apply_due_transport_action(due);
        }
        self.advance_snapshots_to_sample(self.transport.sample());
        self.advance_scalar_transitions_to_sample(self.transport.sample());
        self.advance_instance_gain_transitions_to_sample(self.transport.sample());
        self.finalize_music_transitions();
        self.advance_snapshots(dt);
        self.sync_instances();
        self.reconcile_music_instances();
        self.publish_runtime_state(ctx);
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        self.stop_all();
        self.publish_runtime_state(ctx);
        Ok(())
    }
}
