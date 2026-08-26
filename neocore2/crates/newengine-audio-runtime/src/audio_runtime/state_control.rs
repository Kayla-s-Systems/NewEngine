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
        let mut needs_rebalance = false;
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
                    let source_position = control.get_pos().mul_f32(voice.speed);
                    voice.speed = speed;
                    control.set_speed(speed);
                    let output_position = source_position.div_f32(speed);
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
            let target = Duration::from_secs_f64(seek_seconds.min(86_400.0));
            if let Some(control) = voice.control.as_ref() {
                if let Err(error) = control.try_seek(target) {
                    return AudioVoiceAck {
                        accepted: false,
                        voice_id: request.voice_id,
                        provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                        message: error,
                    };
                }
            }
            voice.virtual_source_position =
                voice.normalized_source_position(target.mul_f32(voice.speed));
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
            voice.spatial = voice.spatial.map(|_| AudioSpatialParams { position });
            if let Some(control) = voice.control.as_ref() {
                if !control.set_emitter_position(position) && voice.spatial.is_some() {
                    return AudioVoiceAck {
                        accepted: false,
                        voice_id: request.voice_id,
                        provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                        message: "position update requires a spatial voice".to_owned(),
                    };
                }
            }
        }
        if let Some(acoustic) = request.acoustic {
            let acoustic = acoustic.sanitized();
            if acoustic != voice.acoustic {
                voice.acoustic = acoustic;
                if let Some(control) = voice.control.as_ref() {
                    control.set_acoustic(acoustic);
                }
                needs_rebalance = true;
            }
        }
        if let Some(environment) = request.environment {
            let environment = environment.sanitized();
            if environment != voice.environment {
                voice.environment = environment;
                if let Some(control) = voice.control.as_ref() {
                    control.set_environment(environment);
                }
            }
        }

        // Release the mutable voice borrow before applying bus/attenuation/acoustic gain.
        let _ = voice;
        if let Some(voice) = self.voices.get(&request.voice_id) {
            if let Some(control) = voice.control.as_ref() {
                control.set_volume(self.voice_audibility(voice));
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
        self.listener = listener.sanitized();
        for voice in self.voices.values() {
            if let Some(control) = voice.control.as_ref() {
                control.update_listener(self.listener);
            }
        }
        self.refresh_voice_gains();
        // Camera -> listener synchronization is presentation-cadence, making it the
        // natural once-per-frame arbitration point for distance/audibility changes.
        self.rebalance_physical_voices();
        self.listener
    }

    fn set_bus_gain(&mut self, request: AudioBusGainRequest) -> AudioBusGainAck {
        let gain = sanitize_gain(request.gain);
        self.bus_gains.insert(request.bus, gain);
        self.refresh_voice_gains();
        self.rebalance_physical_voices();
        AudioBusGainAck {
            accepted: true,
            bus: request.bus,
            gain,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
        }
    }
}
