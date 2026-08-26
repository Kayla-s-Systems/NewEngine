impl AudioRuntimeState {
    fn desired_physical_voices(&self) -> HashSet<u64> {
        let ranks = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| {
                if voice.paused {
                    return None;
                }
                let audibility = self.voice_audibility(voice);
                if !audibility.is_finite() || audibility <= MIN_PHYSICAL_AUDIBILITY {
                    return None;
                }
                Some(VoiceRank {
                    voice_id: *voice_id,
                    priority: voice.priority,
                    audibility,
                    distance: voice.distance_to(self.listener),
                    already_physical: voice.is_physical(),
                })
            })
            .collect::<Vec<_>>();
        select_physical_voice_ids(ranks, self.max_physical_voices)
    }

    fn demote_voice(&mut self, voice_id: u64, now: Instant) {
        let Some(voice) = self.voices.get_mut(&voice_id) else {
            return;
        };
        let Some(control) = voice.control.take() else {
            return;
        };
        voice.virtual_source_position =
            voice.normalized_source_position(control.get_pos().mul_f32(voice.speed));
        control.stop();
        voice.virtual_since = (!voice.paused).then_some(now);
    }

    fn materialize_voice(&mut self, voice_id: u64, now: Instant) -> Result<(), String> {
        let Some(voice) = self.voices.get(&voice_id) else {
            return Err("voice disappeared before materialization".to_owned());
        };
        if voice.control.is_some() {
            return Ok(());
        }
        let source = voice.source.clone();
        let bus = voice.bus;
        let gain = voice.gain;
        let speed = voice.speed;
        let looping = voice.looping;
        let spatial = voice.spatial;
        let attenuation = voice.attenuation.clone();
        let acoustic = voice.acoustic.sanitized();
        let environment_state = voice.environment.sanitized();
        let paused = voice.paused;
        let source_position = voice.current_source_position(now);
        let seek_position = if speed > 0.0 {
            source_position.div_f32(speed)
        } else {
            Duration::ZERO
        };
        let volume = sanitize_gain(gain)
            * self.bus_gain(bus)
            * match (&attenuation, spatial) {
                (Some(attenuation), Some(spatial)) => attenuation
                    .gain_at_distance(distance3(spatial.position, self.listener.position)),
                _ => 1.0,
            }
            * acoustic.transmission_gain;

        self.start_output_init();
        self.poll_output_init();
        if self.output.is_none() {
            return Err(self
                .output_error
                .clone()
                .unwrap_or_else(|| "audio output device is still initializing".to_owned()));
        }

        let mut materialized_stream_stats = None;
        let control = match source {
            VoiceSource::Clip { uri, .. } => {
                let clip_bytes = self.clip_bytes(&uri)?;
                let decoder = Decoder::try_from(Cursor::new(clip_bytes))
                    .map_err(|error| format!("audio decode failed '{uri}': {error}"))?;
                let spectral = SpectralFilterControl::new(acoustic);
                let environment = EnvironmentFilterControl::new(environment_state);
                if let Some(spatial) = spatial {
                    let (left, right) = self.listener.ear_positions();
                    let player = SpatialPlayer::connect_new(
                        self.output.as_ref().expect("output checked").mixer(),
                        spatial.sanitized().position,
                        left,
                        right,
                    );
                    player.set_volume(volume);
                    player.set_speed(speed);
                    if looping {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder.repeat_infinite(), spectral.clone()),
                            environment.clone(),
                        ));
                    } else {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder, spectral.clone()),
                            environment.clone(),
                        ));
                    }
                    let control = VoiceControl::Spatial {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    if should_seek_materialized_voice(seek_position) {
                        control.try_seek(seek_position)?;
                    }
                    control.set_paused(paused);
                    control
                } else {
                    let player =
                        Player::connect_new(self.output.as_ref().expect("output checked").mixer());
                    player.set_volume(volume);
                    player.set_speed(speed);
                    if looping {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder.repeat_infinite(), spectral.clone()),
                            environment.clone(),
                        ));
                    } else {
                        player.append(DynamicEnvironmentSource::new(
                            DynamicSpectralSource::new(decoder, spectral.clone()),
                            environment.clone(),
                        ));
                    }
                    let control = VoiceControl::Flat {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    if should_seek_materialized_voice(seek_position) {
                        control.try_seek(seek_position)?;
                    }
                    control.set_paused(paused);
                    control
                }
            }
            VoiceSource::Stream { uri, buffer } => {
                let reader = RangedAssetReader::new(
                    self.assets.clone(),
                    uri.clone(),
                    buffer.compressed_chunk_bytes,
                    buffer.compressed_cache_bytes,
                );
                let asset_io = reader.stats();
                let (stream, stats) = build_streaming_source(
                    reader,
                    Some(asset_io),
                    looping,
                    buffer,
                    seek_position,
                    &voice_id.to_string(),
                )?;
                materialized_stream_stats = Some(Arc::clone(&stats));
                let spectral = SpectralFilterControl::new(acoustic);
                let environment = EnvironmentFilterControl::new(environment_state);
                if let Some(spatial) = spatial {
                    let (left, right) = self.listener.ear_positions();
                    let player = SpatialPlayer::connect_new(
                        self.output.as_ref().expect("output checked").mixer(),
                        spatial.sanitized().position,
                        left,
                        right,
                    );
                    player.set_volume(volume);
                    player.set_speed(1.0);
                    player.append(DynamicEnvironmentSource::new(
                        DynamicSpectralSource::new(stream, spectral.clone()),
                        environment.clone(),
                    ));
                    let control = VoiceControl::Spatial {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    control.set_paused(paused);
                    control
                } else {
                    let player =
                        Player::connect_new(self.output.as_ref().expect("output checked").mixer());
                    player.set_volume(volume);
                    player.set_speed(1.0);
                    player.append(DynamicEnvironmentSource::new(
                        DynamicSpectralSource::new(stream, spectral.clone()),
                        environment.clone(),
                    ));
                    let control = VoiceControl::Flat {
                        player,
                        spectral: Some(spectral),
                        environment: Some(environment),
                    };
                    control.set_paused(paused);
                    control
                }
            }
            VoiceSource::Tone {
                frequency,
                duration,
            } => {
                let player =
                    Player::connect_new(self.output.as_ref().expect("output checked").mixer());
                player.set_volume(volume);
                player.append(SineWave::new(frequency).take_duration(duration).fade_out(
                    Duration::from_millis((duration.as_millis() as u64 / 2).max(8)),
                ));
                VoiceControl::Flat {
                    player,
                    spectral: None,
                    environment: None,
                }
            }
        };

        let Some(voice) = self.voices.get_mut(&voice_id) else {
            control.stop();
            return Err("voice disappeared during materialization".to_owned());
        };
        voice.control = Some(control);
        voice.stream_stats = materialized_stream_stats;
        voice.virtual_source_position = source_position;
        voice.virtual_since = None;
        Ok(())
    }

    fn rebalance_physical_voices(&mut self) {
        self.prune_finished();
        let now = Instant::now();
        let desired = self.desired_physical_voices();

        let demote = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| {
                (voice.is_physical() && !desired.contains(voice_id)).then_some(*voice_id)
            })
            .collect::<Vec<_>>();
        for voice_id in demote {
            if self
                .voices
                .get(&voice_id)
                .is_some_and(VoiceEntry::virtualizable)
            {
                self.demote_voice(voice_id, now);
            } else {
                let _ = self.remove_voice(voice_id);
            }
        }

        let promote = desired
            .iter()
            .copied()
            .filter(|voice_id| {
                self.voices
                    .get(voice_id)
                    .is_some_and(VoiceEntry::is_virtual)
            })
            .collect::<Vec<_>>();
        for voice_id in promote {
            match self.materialize_voice(voice_id, now) {
                Ok(()) => {
                    self.materialization_errors.remove(&voice_id);
                }
                Err(error) => {
                    self.materialization_errors.insert(voice_id, error.clone());
                    newengine_ulog_api::ulog::warn!(
                        "audio virtualization: promote failed voice_id={} err='{}'",
                        voice_id,
                        error
                    );
                }
            }
        }

        // Non-virtualizable logical voices are valid only while physically realized.
        let invalid = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| {
                (voice.is_virtual() && !voice.virtualizable()).then_some(*voice_id)
            })
            .collect::<Vec<_>>();
        for voice_id in invalid {
            let _ = self.remove_voice(voice_id);
        }

        debug_assert!(
            self.voices
                .values()
                .filter(|voice| voice.is_physical())
                .count()
                <= self.max_physical_voices
        );
    }
}
