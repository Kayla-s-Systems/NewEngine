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
                    budget: voice.voice_budget.clone(),
                })
            })
            .collect::<Vec<_>>();
        select_physical_voice_ids(
            ranks,
            self.max_physical_voices,
            &self.voice_budget_reservations,
        )
    }

    fn demote_voice(&mut self, voice_id: u64, now: Instant) {
        let was_stream = {
            let Some(voice) = self.voices.get_mut(&voice_id) else {
                return;
            };
            if voice.control.is_none() {
                return;
            }
            let absolute_source_position = voice.current_source_position(now);
            let control = voice.control.take().expect("control checked above");
            let was_stream = matches!(voice.source, VoiceSource::Stream { .. });
            voice.virtual_source_position = absolute_source_position;
            control.stop();
            // Stream decoder/ring/cache state is physical residency only. The logical
            // source timeline stays on VoiceEntry and survives this destruction.
            voice.stream_stats = None;
            voice.physical_source_origin = Duration::ZERO;
            voice.virtual_since = (!voice.paused).then_some(now);
            was_stream
        };
        if was_stream {
            self.stream_demotions = self.stream_demotions.saturating_add(1);
        }
    }

    fn materialize_voice(&mut self, voice_id: u64, now: Instant) -> Result<(), String> {
        let Some(voice) = self.voices.get(&voice_id) else {
            return Err("voice disappeared before materialization".to_owned());
        };
        if voice.control.is_some() {
            return Ok(());
        }
        let source = voice.source.clone();
        let route = voice.route.clone();
        let gain = voice.gain;
        let speed = voice.effective_speed();
        let looping = voice.looping;
        let spatial = voice.spatial;
        let attenuation = voice.attenuation.clone();
        let acoustic = voice.propagated_acoustic();
        let environment_state = voice.environment.sanitized();
        let paused = voice.paused;
        let render_start_sample = voice.render_start_sample;
        let source_position = voice.current_source_position(now);
        let seek_position = if speed > 0.0 {
            source_position.div_f32(speed)
        } else {
            Duration::ZERO
        };
        let volume = sanitize_gain(gain)
            * self.route_gain(&route)
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
        let late_binding = self.room_bus_binding_for_environment(environment_state, volume);

        let render_graph = self
            .render_graph
            .clone()
            .ok_or_else(|| "native block render graph is not initialized".to_owned())?;
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
                    let spatial_control =
                        SpatialMixControl::new(spatial.sanitized().position, left, right);
                    let mut rendered: Box<dyn Source + Send> = if looping {
                        let mono = ChannelVolume::new(
                            DynamicSpectralSource::new(decoder.repeat_infinite(), spectral.clone()),
                            vec![1.0],
                        );
                        Box::new(DynamicSpatialEnvironmentSource::new_with_late_binding(
                            mono,
                            environment.clone(),
                            spatial_control.clone(),
                            late_binding.clone(),
                        ))
                    } else {
                        let mono = ChannelVolume::new(
                            DynamicSpectralSource::new(decoder, spectral.clone()),
                            vec![1.0],
                        );
                        Box::new(DynamicSpatialEnvironmentSource::new_with_late_binding(
                            mono,
                            environment.clone(),
                            spatial_control.clone(),
                            late_binding.clone(),
                        ))
                    };
                    let initial_output_position = if should_seek_materialized_voice(seek_position) {
                        rendered.try_seek(seek_position).map_err(|error| {
                            format!("audio voice seek failed during materialization: {error}")
                        })?;
                        seek_position
                    } else {
                        Duration::ZERO
                    };
                    let render = render_graph.add_boxed_source(
                        rendered,
                        volume,
                        speed,
                        paused,
                        initial_output_position,
                        render_start_sample,
                    )?;
                    VoiceControl::Spatial {
                        render,
                        spatial: spatial_control,
                        spectral: Some(spectral),
                        environment: Some(environment),
                        late_binding: late_binding.clone(),
                    }
                } else {
                    let mut rendered: Box<dyn Source + Send> = if looping {
                        Box::new(DynamicEnvironmentSource::new_with_late_binding(
                            DynamicSpectralSource::new(decoder.repeat_infinite(), spectral.clone()),
                            environment.clone(),
                            late_binding.clone(),
                        ))
                    } else {
                        Box::new(DynamicEnvironmentSource::new_with_late_binding(
                            DynamicSpectralSource::new(decoder, spectral.clone()),
                            environment.clone(),
                            late_binding.clone(),
                        ))
                    };
                    let initial_output_position = if should_seek_materialized_voice(seek_position) {
                        rendered.try_seek(seek_position).map_err(|error| {
                            format!("audio voice seek failed during materialization: {error}")
                        })?;
                        seek_position
                    } else {
                        Duration::ZERO
                    };
                    let render = render_graph.add_boxed_source(
                        rendered,
                        volume,
                        speed,
                        paused,
                        initial_output_position,
                        render_start_sample,
                    )?;
                    VoiceControl::Flat {
                        render,
                        spectral: Some(spectral),
                        environment: Some(environment),
                        late_binding: late_binding.clone(),
                    }
                }
            }
            VoiceSource::Stream {
                uri,
                buffer,
                source_duration: _,
            } => {
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
                    let spatial_control =
                        SpatialMixControl::new(spatial.sanitized().position, left, right);
                    let mono = ChannelVolume::new(
                        DynamicSpectralSource::new(stream, spectral.clone()),
                        vec![1.0],
                    );
                    let rendered: Box<dyn Source + Send> = Box::new(
                        DynamicSpatialEnvironmentSource::new_with_late_binding(
                            mono,
                            environment.clone(),
                            spatial_control.clone(),
                            late_binding.clone(),
                        ),
                    );
                    let render = render_graph.add_boxed_source(
                        rendered,
                        volume,
                        1.0,
                        paused,
                        Duration::ZERO,
                        render_start_sample,
                    )?;
                    VoiceControl::Spatial {
                        render,
                        spatial: spatial_control,
                        spectral: Some(spectral),
                        environment: Some(environment),
                        late_binding: late_binding.clone(),
                    }
                } else {
                    let rendered: Box<dyn Source + Send> = Box::new(
                        DynamicEnvironmentSource::new_with_late_binding(
                            DynamicSpectralSource::new(stream, spectral.clone()),
                            environment.clone(),
                            late_binding.clone(),
                        ),
                    );
                    let render = render_graph.add_boxed_source(
                        rendered,
                        volume,
                        1.0,
                        paused,
                        Duration::ZERO,
                        render_start_sample,
                    )?;
                    VoiceControl::Flat {
                        render,
                        spectral: Some(spectral),
                        environment: Some(environment),
                        late_binding: late_binding.clone(),
                    }
                }
            }
            VoiceSource::Tone {
                frequency,
                duration,
            } => {
                let rendered = SineWave::new(frequency).take_duration(duration).fade_out(
                    Duration::from_millis((duration.as_millis() as u64 / 2).max(8)),
                );
                let render = render_graph.add_boxed_source(
                    Box::new(rendered),
                    volume,
                    1.0,
                    paused,
                    Duration::ZERO,
                    render_start_sample,
                )?;
                VoiceControl::Flat {
                    render,
                    spectral: None,
                    environment: None,
                    late_binding: None,
                }
            }
        };

        let Some(voice) = self.voices.get_mut(&voice_id) else {
            control.stop();
            return Err("voice disappeared during materialization".to_owned());
        };
        let materialized_stream = matches!(voice.source, VoiceSource::Stream { .. });
        voice.control = Some(control);
        voice.stream_stats = materialized_stream_stats;
        voice.physical_source_origin = if materialized_stream {
            source_position
        } else {
            Duration::ZERO
        };
        voice.virtual_source_position = source_position;
        voice.virtual_since = None;
        if materialized_stream {
            self.stream_promotions = self.stream_promotions.saturating_add(1);
        }
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
