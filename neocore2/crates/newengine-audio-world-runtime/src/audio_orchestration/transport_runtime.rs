impl AudioOrchestrationRuntimeModule {
    fn publish_transport_markers(&self, markers: Vec<AudioTransportMarkerOccurrence>) {
        if markers.is_empty() {
            return;
        }
        let mut events = self.handle.transport_events.lock();
        for marker in markers {
            if events.len() >= self.handle.config.transport_event_capacity {
                events.pop_front();
                self.handle
                    .dropped_transport_events
                    .fetch_add(1, Ordering::Relaxed);
            }
            events.push_back(marker);
        }
    }

    fn refresh_provider_clock(&mut self) {
        let should_query = self.transport.has_pending_actions()
            || !self.instances.is_empty()
            || !self.music_sessions.is_empty()
            || self.provider_clock_anchor.is_some();
        if !should_query {
            self.provider_clock = None;
            return;
        }

        match audio_render_clock() {
            Ok(Some(clock)) if clock.ready && clock.sample_rate > 0 => {
                let reset_anchor = self.provider_clock_anchor.is_none_or(|anchor| {
                    anchor.provider_rate != clock.sample_rate
                        || clock.sample < anchor.provider_sample
                });
                if reset_anchor {
                    self.provider_clock_anchor = Some(ProviderClockAnchor {
                        transport_sample: self.transport.sample(),
                        provider_sample: clock.sample,
                        provider_rate: clock.sample_rate,
                    });
                }
                self.provider_clock = Some(clock);
            }
            Ok(_) => {
                self.provider_clock = None;
            }
            Err(error) => {
                self.provider_clock = None;
                newengine_ulog_api::ulog::trace!(
                    "audio transport: provider render clock unavailable err='{}'",
                    error
                );
            }
        }
    }

    fn scale_samples(samples: u64, from_rate: u32, to_rate: u32) -> Option<u64> {
        if from_rate == 0 || to_rate == 0 {
            return None;
        }
        let numerator = u128::from(samples)
            .saturating_mul(u128::from(to_rate))
            .saturating_add(u128::from(from_rate / 2));
        Some((numerator / u128::from(from_rate)).min(u128::from(u64::MAX)) as u64)
    }

    fn provider_sample_for_transport(&self, transport_sample: u64) -> Option<u64> {
        let anchor = self.provider_clock_anchor?;
        if transport_sample < anchor.transport_sample {
            return None;
        }
        let delta = transport_sample - anchor.transport_sample;
        let provider_delta =
            Self::scale_samples(delta, self.transport.sample_rate(), anchor.provider_rate)?;
        Some(anchor.provider_sample.saturating_add(provider_delta))
    }

    fn provider_duration_for_transport(&self, transport_samples: u64) -> Option<u64> {
        let anchor = self.provider_clock_anchor?;
        let scaled = Self::scale_samples(
            transport_samples,
            self.transport.sample_rate(),
            anchor.provider_rate,
        )?;
        Some(if transport_samples > 0 {
            scaled.max(1)
        } else {
            0
        })
    }

    fn provider_target_has_prearm_lead(&self, provider_sample: u64) -> bool {
        let Some(clock) = self.provider_clock else {
            return false;
        };
        let lead = u64::from(clock.block_frames.max(1))
            .saturating_mul(u64::from(self.handle.config.provider_prearm_blocks));
        provider_sample >= clock.sample.saturating_add(lead)
    }

    fn advance_transport_clock(
        &mut self,
        dt: f32,
    ) -> (Vec<AudioTransportMarkerOccurrence>, Vec<DueTransportAction>) {
        let Some(clock) = self.provider_clock else {
            return self.transport.advance_seconds(dt);
        };
        let Some(anchor) = self.provider_clock_anchor else {
            return self.transport.advance_seconds(dt);
        };
        if clock.sample < anchor.provider_sample || clock.sample_rate != anchor.provider_rate {
            return self.transport.advance_seconds(dt);
        }
        let provider_delta = clock.sample - anchor.provider_sample;
        let Some(transport_delta) = Self::scale_samples(
            provider_delta,
            anchor.provider_rate,
            self.transport.sample_rate(),
        ) else {
            return self.transport.advance_seconds(dt);
        };
        let target = anchor.transport_sample.saturating_add(transport_delta);
        self.transport
            .advance_samples(target.saturating_sub(self.transport.sample()))
    }

    fn cancel_provider_voice_schedule(&self, voice_ids: &[u64], schedule_id: u64) {
        for voice_id in voice_ids.iter().copied() {
            let request = AudioVoiceRenderScheduleRequest {
                voice_id,
                at_sample: self.provider_clock.map_or(0, |clock| clock.sample),
                schedule_id,
                action: AudioVoiceRenderAction::Cancel,
            };
            let _ = schedule_audio_voice_render(&request);
        }
    }

    fn cancel_prearmed_transport_action(&mut self, action_id: AudioTransportActionId) {
        let Some(prearmed) = self.prearmed_transport_actions.remove(&action_id) else {
            return;
        };
        match prearmed {
            PrearmedTransportAction::Play { instance_id }
            | PrearmedTransportAction::PlayStream { instance_id } => {
                self.stop_instance(instance_id);
            }
            PrearmedTransportAction::Gain {
                instance_id,
                voice_ids,
                schedule_id,
                ..
            } => {
                self.cancel_provider_voice_schedule(&voice_ids, schedule_id);
                self.provider_gain_ramp_until.remove(&instance_id);
            }
            PrearmedTransportAction::Stop {
                voice_ids,
                schedule_id,
                ..
            } => self.cancel_provider_voice_schedule(&voice_ids, schedule_id),
        }
    }

    fn try_prearm_transport_action(&mut self, pending: PendingTransportAction) {
        if self.prearmed_transport_actions.contains_key(&pending.id) {
            return;
        }
        let Some(provider_sample) = self.provider_sample_for_transport(pending.intended_sample)
        else {
            return;
        };
        if !self.provider_target_has_prearm_lead(provider_sample) {
            return;
        }

        match pending.action {
            AudioTransportAction::Play {
                instance_id,
                object_id,
                request,
            } => {
                self.play_instance(
                    instance_id,
                    object_id,
                    request,
                    pending.intended_sample,
                    self.transport.sample(),
                    Some(provider_sample),
                );
                if self
                    .instances
                    .get(&instance_id)
                    .is_some_and(|instance| instance.render_armed)
                {
                    self.prearmed_transport_actions
                        .insert(pending.id, PrearmedTransportAction::Play { instance_id });
                }
            }
            AudioTransportAction::PlayStream {
                instance_id,
                object_id,
                request,
            } => {
                self.play_stream_instance(
                    instance_id,
                    object_id,
                    *request,
                    pending.intended_sample,
                    self.transport.sample(),
                    Some(provider_sample),
                );
                if self
                    .instances
                    .get(&instance_id)
                    .is_some_and(|instance| instance.render_armed)
                {
                    self.prearmed_transport_actions.insert(
                        pending.id,
                        PrearmedTransportAction::PlayStream { instance_id },
                    );
                }
            }
            AudioTransportAction::TransitionInstanceGain {
                instance_id,
                target_gain,
                duration_samples,
            } => {
                let Some(instance) = self.instances.get(&instance_id).cloned() else {
                    return;
                };
                if instance.render_armed {
                    return;
                }
                let Some(object) = self.objects.get(&instance.object_id) else {
                    return;
                };
                let route_gain = self.route_gain(&instance.route);
                let target_voice_gain = newengine_audio_api::sanitize_gain(
                    object.state.gain * target_gain * route_gain,
                );
                let Some(provider_duration) =
                    self.provider_duration_for_transport(duration_samples)
                else {
                    return;
                };
                let schedule_id = pending.id.0;
                let mut armed = Vec::with_capacity(instance.voice_ids.len());
                for voice_id in instance.voice_ids.iter().copied() {
                    let request = AudioVoiceRenderScheduleRequest {
                        voice_id,
                        at_sample: provider_sample,
                        schedule_id,
                        action: AudioVoiceRenderAction::GainRamp {
                            target_gain: target_voice_gain,
                            duration_samples: provider_duration,
                        },
                    };
                    match schedule_audio_voice_render(&request) {
                        Ok(Some(ack)) if ack.accepted => armed.push(voice_id),
                        _ => {
                            self.cancel_provider_voice_schedule(&armed, schedule_id);
                            return;
                        }
                    }
                }
                if !armed.is_empty() {
                    self.prearmed_transport_actions.insert(
                        pending.id,
                        PrearmedTransportAction::Gain {
                            instance_id,
                            voice_ids: armed,
                            schedule_id,
                            end_transport_sample: pending
                                .intended_sample
                                .saturating_add(duration_samples),
                        },
                    );
                }
            }
            AudioTransportAction::StopInstance { instance_id } => {
                let Some(instance) = self.instances.get(&instance_id).cloned() else {
                    return;
                };
                let schedule_id = pending.id.0;
                let mut armed = Vec::with_capacity(instance.voice_ids.len());
                for voice_id in instance.voice_ids.iter().copied() {
                    let request = AudioVoiceRenderScheduleRequest {
                        voice_id,
                        at_sample: provider_sample,
                        schedule_id,
                        action: AudioVoiceRenderAction::Stop,
                    };
                    match schedule_audio_voice_render(&request) {
                        Ok(Some(ack)) if ack.accepted => armed.push(voice_id),
                        _ => {
                            self.cancel_provider_voice_schedule(&armed, schedule_id);
                            return;
                        }
                    }
                }
                if !armed.is_empty() {
                    self.prearmed_transport_actions.insert(
                        pending.id,
                        PrearmedTransportAction::Stop {
                            instance_id,
                            voice_ids: armed,
                            schedule_id,
                        },
                    );
                }
            }
            _ => {}
        }
    }

    fn prearm_pending_transport_actions(&mut self) {
        for pending in self.transport.pending_actions() {
            self.try_prearm_transport_action(pending);
        }
    }

    fn apply_due_transport_action(&mut self, due: DueTransportAction) {
        if due.lateness_samples > 0 {
            newengine_ulog_api::ulog::trace!(
                "audio transport: late logical dispatch action_id={} intended_sample={} dispatch_sample={} lateness_samples={}",
                due.id.0,
                due.intended_sample,
                due.dispatch_sample,
                due.lateness_samples
            );
        }
        let prearmed = self.prearmed_transport_actions.remove(&due.id);
        match due.action {
            AudioTransportAction::Play {
                instance_id,
                object_id,
                request,
            } => {
                if matches!(
                    prearmed,
                    Some(PrearmedTransportAction::Play {
                        instance_id: armed
                    }) if armed == instance_id
                ) {
                    if let Some(instance) = self.instances.get_mut(&instance_id) {
                        instance.render_armed = false;
                        instance.transport_dispatch_sample = due.dispatch_sample;
                    }
                } else {
                    self.play_instance(
                        instance_id,
                        object_id,
                        request,
                        due.intended_sample,
                        due.dispatch_sample,
                        None,
                    );
                }
            }
            AudioTransportAction::PlayStream {
                instance_id,
                object_id,
                request,
            } => {
                if matches!(
                    prearmed,
                    Some(PrearmedTransportAction::PlayStream {
                        instance_id: armed
                    }) if armed == instance_id
                ) {
                    if let Some(instance) = self.instances.get_mut(&instance_id) {
                        instance.render_armed = false;
                        instance.transport_dispatch_sample = due.dispatch_sample;
                    }
                } else {
                    self.play_stream_instance(
                        instance_id,
                        object_id,
                        *request,
                        due.intended_sample,
                        due.dispatch_sample,
                        None,
                    );
                }
            }
            AudioTransportAction::StopInstance { instance_id } => {
                if let Some(PrearmedTransportAction::Stop {
                    instance_id: armed_instance,
                    ..
                }) = prearmed
                {
                    debug_assert_eq!(armed_instance, instance_id);
                }
                self.stop_instance(instance_id);
            }
            AudioTransportAction::SetScalar {
                target,
                name,
                value,
            } => self.set_scalar(target, name, value),
            AudioTransportAction::SetSwitch {
                target,
                name,
                value,
            } => self.set_switch(target, name, value),
            AudioTransportAction::TransitionScalar {
                target,
                name,
                target_value,
                duration_samples,
            } => self.transition_scalar_samples(
                target,
                name,
                target_value,
                due.intended_sample,
                duration_samples,
            ),
            AudioTransportAction::TransitionInstanceGain {
                instance_id,
                target_gain,
                duration_samples,
            } => {
                let exact_provider_end = match prearmed {
                    Some(PrearmedTransportAction::Gain {
                        instance_id: armed,
                        end_transport_sample,
                        ..
                    }) if armed == instance_id => Some(end_transport_sample),
                    _ => None,
                };
                self.transition_instance_gain_samples(
                    instance_id,
                    target_gain,
                    due.intended_sample,
                    duration_samples,
                );
                if let Some(end_sample) = exact_provider_end {
                    self.provider_gain_ramp_until
                        .insert(instance_id, end_sample);
                }
            }
            AudioTransportAction::TransitionSnapshot {
                snapshot,
                target_weight,
                duration_samples,
            } => self.transition_snapshot_samples(
                &snapshot,
                target_weight,
                due.intended_sample,
                duration_samples,
            ),
        }
    }
}
