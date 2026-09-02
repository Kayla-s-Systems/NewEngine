impl AudioRuntimeState {
    fn stop_voice(&mut self, request: AudioStopVoiceRequest) -> AudioVoiceAck {
        self.prune_finished();
        let accepted = self.remove_voice(request.voice_id).is_some();
        if accepted {
            self.rebalance_physical_voices();
        }
        AudioVoiceAck {
            accepted,
            voice_id: request.voice_id,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            message: if accepted {
                String::new()
            } else {
                "voice not found".to_owned()
            },
        }
    }

    fn update_voice(&mut self, request: AudioVoiceUpdateRequest) -> AudioVoiceAck {
        self.prune_finished();
        let now = Instant::now();
        let listener = self.listener;
        let listener_velocity = self.listener_velocity;
        let mut needs_rebalance = false;
        let mut changed_environment = None;
        let Some(voice) = self.voices.get_mut(&request.voice_id) else {
            return AudioVoiceAck {
                accepted: false,
                voice_id: request.voice_id,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                message: "voice not found".to_owned(),
            };
        };

        if let Some(gain) = request.gain {
            voice.gain = sanitize_gain(gain);
        }
        if let Some(speed) = request.speed {
            if matches!(voice.source, VoiceSource::Stream { .. }) {
                return AudioVoiceAck {
                    accepted: false,
                    voice_id: request.voice_id,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    message: "streaming voices do not support runtime speed changes".to_owned(),
                };
            }
            let speed = sanitize_speed(speed);
            if (speed - voice.speed).abs() > f32::EPSILON {
                if let Some(control) = voice.control.as_ref() {
                    let old_effective_speed = voice.effective_speed();
                    let source_position = control.get_pos().mul_f32(old_effective_speed);
                    voice.speed = speed;
                    let new_effective_speed = voice.effective_speed();
                    control.set_speed(new_effective_speed);
                    let output_position = source_position.div_f32(new_effective_speed);
                    if let Err(error) = control.try_seek(output_position) {
                        return AudioVoiceAck {
                            accepted: false,
                            voice_id: request.voice_id,
                            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                            message: error,
                        };
                    }
                } else {
                    voice.freeze_virtual_timeline(now);
                    voice.speed = speed;
                    voice.resume_virtual_timeline(now);
                }
            }
        }
        if let Some(seek_seconds) = request.seek_seconds {
            if !seek_seconds.is_finite() || seek_seconds < 0.0 {
                return AudioVoiceAck {
                    accepted: false,
                    voice_id: request.voice_id,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    message: "voice seek_seconds must be finite and non-negative".to_owned(),
                };
            }
            let requested_target = Duration::from_secs_f64(seek_seconds.min(86_400.0));
            let stream_source = matches!(voice.source, VoiceSource::Stream { .. });
            let target = if stream_source {
                voice.normalized_source_position(requested_target)
            } else {
                requested_target
            };
            if let Some(control) = voice.control.as_ref() {
                if let Err(error) = control.try_seek(target) {
                    return AudioVoiceAck {
                        accepted: false,
                        voice_id: request.voice_id,
                        provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                        message: error,
                    };
                }
                if stream_source {
                    // The physical render node clock is reset to the absolute seek target after a successful
                    // seek, so no external origin is required until the next rematerialization.
                    voice.physical_source_origin = Duration::ZERO;
                }
            }
            voice.virtual_source_position = voice.normalized_source_position(if stream_source {
                target
            } else {
                target.mul_f32(voice.speed)
            });
            voice.virtual_since = (!voice.paused).then_some(now);
        }
        if let Some(paused) = request.paused {
            if paused != voice.paused {
                voice.freeze_virtual_timeline(now);
                voice.paused = paused;
                if let Some(control) = voice.control.as_ref() {
                    control.set_paused(paused);
                } else {
                    voice.resume_virtual_timeline(now);
                }
                needs_rebalance = true;
            }
        }
        if let Some(position) = request.position {
            let position = sanitize_position(position);
            let was_spatial = voice.spatial.is_some();
            voice.update_emitter_motion(position, now);
            if let Some(control) = voice.control.as_ref() {
                if !control.set_emitter_position(position) && was_spatial {
                    return AudioVoiceAck {
                        accepted: false,
                        voice_id: request.voice_id,
                        provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                        message: "position update requires a spatial voice".to_owned(),
                    };
                }
            }
            voice.refresh_propagation(listener, listener_velocity);
            needs_rebalance = true;
        }
        if let Some(acoustic) = request.acoustic {
            let acoustic = acoustic.sanitized();
            if acoustic != voice.acoustic {
                voice.acoustic = acoustic;
                if let Some(control) = voice.control.as_ref() {
                    control.set_acoustic(voice.propagated_acoustic());
                }
                needs_rebalance = true;
            }
        }
        if let Some(environment) = request.environment {
            let environment = environment.sanitized();
            if environment != voice.environment {
                voice.environment = environment;
                changed_environment = Some(environment);
                if let Some(control) = voice.control.as_ref() {
                    control.set_environment(environment);
                }
            }
        }

        // A logical voice can be accepted while the lazy native output is still initializing.
        // Revisit physical arbitration on every subsequent update until that voice materializes;
        // otherwise non-spatial looping ambience can remain virtual forever because none of its
        // steady-state fields necessarily changes after the initial play request.
        needs_rebalance |= voice.is_virtual();

        // Release the mutable voice borrow before room-bus allocation/rebinding and gain updates.
        let _ = voice;
        if let Some(environment) = changed_environment {
            let (late_binding, physical, supports_environment) = self
                .voices
                .get(&request.voice_id)
                .map(|voice| {
                    (
                        voice
                            .control
                            .as_ref()
                            .and_then(VoiceControl::late_binding)
                            .cloned(),
                        voice.is_physical(),
                        !matches!(voice.source, VoiceSource::Tone { .. }),
                    )
                })
                .unwrap_or((None, false, false));
            let wants_shared = environment.source_send.room_bus_id != 0
                || environment.listener_send.room_bus_id != 0;
            let mut rematerialize = false;
            if let Some(binding) = late_binding.as_ref() {
                rematerialize = !self.rebind_room_bus_voice(binding, environment);
            } else if physical && supports_environment && wants_shared {
                rematerialize = true;
            }
            if rematerialize && physical && supports_environment {
                self.demote_voice(request.voice_id, now);
                if let Err(error) = self.materialize_voice(request.voice_id, now) {
                    self.materialization_errors.insert(request.voice_id, error);
                } else {
                    self.materialization_errors.remove(&request.voice_id);
                }
            }
        }
        if let Some(voice) = self.voices.get(&request.voice_id) {
            if let Some(control) = voice.control.as_ref() {
                control.set_volume(self.voice_output_gain(voice));
            }
        }
        if needs_rebalance {
            self.rebalance_physical_voices();
        }
        AudioVoiceAck {
            accepted: true,
            voice_id: request.voice_id,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            message: String::new(),
        }
    }

    fn set_listener(&mut self, listener: AudioListenerState) -> AudioListenerState {
        let now = Instant::now();
        let listener = listener.sanitized();
        let dt = self
            .listener_updated_at
            .map(|last| now.saturating_duration_since(last).as_secs_f32())
            .unwrap_or(0.0);
        let target_velocity = estimate_velocity(self.listener.position, listener.position, dt);
        self.listener_velocity = smooth_velocity(self.listener_velocity, target_velocity);
        self.listener_updated_at = Some(now);
        self.listener = listener;
        for voice in self.voices.values_mut() {
            voice.refresh_propagation(self.listener, self.listener_velocity);
            if let Some(control) = voice.control.as_ref() {
                control.update_listener(self.listener);
            }
        }
        self.refresh_voice_gains();
        // Camera -> listener synchronization is presentation-cadence, making it the
        // natural once-per-frame propagation + arbitration point.
        self.rebalance_physical_voices();
        self.listener
    }

    fn set_route_gain(&mut self, mut request: AudioRouteGainRequest) -> AudioRouteGainAck {
        request.route.0 = request.route.0.trim().to_owned();
        let gain = sanitize_gain(request.gain);
        if request.route.0.is_empty() || request.route.validate().is_err() {
            return AudioRouteGainAck {
                accepted: false,
                route: request.route,
                gain,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            };
        }
        self.route_gains.insert(request.route.clone(), gain);
        self.refresh_voice_gains();
        self.rebalance_physical_voices();
        AudioRouteGainAck {
            accepted: true,
            route: request.route,
            gain,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
        }
    }
}
