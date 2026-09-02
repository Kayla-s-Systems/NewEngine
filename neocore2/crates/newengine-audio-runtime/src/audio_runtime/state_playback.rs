impl AudioRuntimeState {
    fn play_clip(&mut self, request: AudioPlayRequest) -> Result<AudioPlayAck, String> {
        let policy_instance_id = self.alloc_policy_instance_id();
        self.play_clip_with_policy(
            request,
            AudioVoicePolicy::default(),
            None,
            policy_instance_id,
            Duration::ZERO,
        )
    }

    fn play_clip_with_policy(
        &mut self,
        request: AudioPlayRequest,
        policy: AudioVoicePolicy,
        scope_id: Option<u64>,
        policy_instance_id: u64,
        authored_initial_position: Duration,
    ) -> Result<AudioPlayAck, String> {
        self.prune_finished();
        let request = request.sanitized();
        if !request.route.0.is_empty() {
            request.route.validate()?;
            if !self.route_is_configured(&request.route) {
                return Ok(AudioPlayAck { accepted:false, provider:NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(), voice_id:None, voice_ids:Vec::new(), message:format!("audio route '{}' is not installed by project AudioMixGraph", request.route.0), virtualized:false, diagnostics:Vec::new() });
            }
        }
        if let Err(message) = self.validate_render_start_sample(request.render_start_sample) {
            return Ok(AudioPlayAck { accepted:false, provider:NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(), voice_id:None, voice_ids:Vec::new(), message, virtualized:false, diagnostics:Vec::new() });
        }
        let uri = normalize_vfs_path(&request.clip.uri)?;
        let source_duration = self.clip_source_duration(&uri)?;
        if !request.looping
            && source_duration.is_some_and(|duration| {
                !duration.is_zero() && authored_initial_position >= duration
            })
        {
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                voice_ids: Vec::new(),
                message: "transport start offset is at or beyond source duration".to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        }
        let initial_position = normalize_timeline_position(
            authored_initial_position,
            source_duration,
            request.looping,
        );
        let policy = policy.sanitized()?;
        let scope_key = Self::concurrency_scope_key(&policy, scope_id)?;
        match self.admit_voice_policy(&policy, scope_id, policy_instance_id)? {
            VoicePolicyAdmission::Rejected { reason } => {
                return Ok(AudioPlayAck {
                    accepted: false,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    voice_id: None,
                    voice_ids: Vec::new(),
                    message: reason,
                    virtualized: false,
                    diagnostics: Vec::new(),
                });
            }
            VoicePolicyAdmission::Accepted { stolen_instances } => {
                if !stolen_instances.is_empty() {
                    newengine_ulog_api::ulog::trace!(
                        "audio voice policy: admitted group='{}' scope={:?} limit={} steal_rule={:?} stolen_instances={:?}",
                        policy.group,
                        policy.scope,
                        policy.limit,
                        policy.steal_rule,
                        stolen_instances
                    );
                }
            }
        }

        let voice_id = self.alloc_voice_id();
        let now = Instant::now();
        self.voices.insert(
            voice_id,
            VoiceEntry {
                control: None,
                source: VoiceSource::Clip {
                    uri,
                    source_duration,
                },
                route: request.route.clone(),
                gain: request.gain,
                speed: sanitize_speed(request.speed),
                looping: request.looping,
                spatial: request.spatial,
                attenuation: request.attenuation,
                acoustic: request.acoustic.sanitized(),
                propagation: propagation_state(
                    self.listener,
                    self.listener_velocity,
                    request.spatial,
                    [0.0; 3],
                ),
                emitter_velocity: [0.0; 3],
                last_spatial_update: request.spatial.map(|_| now),
                environment: request.environment.sanitized(),
                stream_stats: None,
                physical_source_origin: Duration::ZERO,
                concurrency_group: policy.group,
                concurrency_scope: policy.scope,
                concurrency_scope_id: scope_key,
                policy_instance_id,
                voice_budget: policy.budget,
                priority: policy.priority,
                paused: false,
                render_start_sample: request.render_start_sample,
                virtual_source_position: initial_position,
                virtual_since: Some(now),
            },
        );
        self.rebalance_physical_voices();

        if request.render_start_sample.is_some()
            && self.voices.get(&voice_id).is_some_and(VoiceEntry::is_virtual)
        {
            let _ = self.remove_voice(voice_id);
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                voice_ids: Vec::new(),
                message: "exact render-scheduled voice requires a physical voice slot".to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        }

        let Some(voice) = self.voices.get(&voice_id) else {
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                voice_ids: Vec::new(),
                message: "physical voice budget exhausted for a non-virtualizable source"
                    .to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        };
        Ok(AudioPlayAck {
            accepted: true,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            voice_id: Some(voice_id),
            voice_ids: vec![voice_id],
            message: if voice.is_virtual() {
                "voice accepted as virtual; awaiting a physical mixer slot".to_owned()
            } else {
                String::new()
            },
            virtualized: voice.is_virtual(),
            diagnostics: Vec::new(),
        })
    }

    fn stream_source_metadata(
        &mut self,
        uri: &str,
        buffer: AudioStreamBufferConfig,
    ) -> Result<StreamSourceMetadata, String> {
        if let Some(metadata) = self.stream_metadata.get(uri).copied() {
            return Ok(metadata);
        }
        let reader = RangedAssetReader::new(
            self.assets.clone(),
            uri.to_owned(),
            buffer.compressed_chunk_bytes,
            buffer.compressed_cache_bytes,
        );
        let metadata = probe_stream_source_metadata(reader)?;
        self.stream_metadata.insert(uri.to_owned(), metadata);
        Ok(metadata)
    }

    fn play_stream(&mut self, request: AudioStreamPlayRequest) -> Result<AudioPlayAck, String> {
        self.prune_finished();
        let request = request.sanitized();
        if !request.route.0.is_empty() {
            request.route.validate()?;
            if !self.route_is_configured(&request.route) {
                return Ok(AudioPlayAck { accepted:false, provider:NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(), voice_id:None, voice_ids:Vec::new(), message:format!("audio route '{}' is not installed by project AudioMixGraph", request.route.0), virtualized:false, diagnostics:Vec::new() });
            }
        }
        if let Err(message) = self.validate_render_start_sample(request.render_start_sample) {
            return Ok(AudioPlayAck { accepted:false, provider:NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(), voice_id:None, voice_ids:Vec::new(), message, virtualized:false, diagnostics:Vec::new() });
        }
        if request.version != 1 {
            return Err(format!(
                "unsupported AudioStreamPlayRequest version {}",
                request.version
            ));
        }
        if request.clip.uri.trim().is_empty() {
            return Err("streaming audio requires a non-empty VFS clip uri".to_owned());
        }
        let uri = normalize_vfs_path(&request.clip.uri)?;
        let metadata = self.stream_source_metadata(&uri, request.buffer)?;
        let authored_start = Duration::from_secs_f64(request.start_seconds);
        if !request.looping
            && metadata
                .source_duration
                .is_some_and(|duration| !duration.is_zero() && authored_start >= duration)
        {
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                voice_ids: Vec::new(),
                message: "stream start position is at or beyond source duration".to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        }
        let initial_position = normalize_timeline_position(
            authored_start,
            metadata.source_duration,
            request.looping,
        );
        let policy = AudioVoicePolicy {
            group: request.concurrency_group.clone(),
            limit: request.concurrency_limit,
            scope: request.concurrency_scope,
            steal_rule: request.steal_rule,
            budget: request.voice_budget.clone(),
            priority: request.priority,
        }
        .sanitized()?;
        let policy_instance_id = self.alloc_policy_instance_id();
        let scope_key = Self::concurrency_scope_key(&policy, request.scope_id)?;
        match self.admit_voice_policy(&policy, request.scope_id, policy_instance_id)? {
            VoicePolicyAdmission::Rejected { reason } => {
                return Ok(AudioPlayAck {
                    accepted: false,
                    provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                    voice_id: None,
                    voice_ids: Vec::new(),
                    message: reason,
                    virtualized: false,
                    diagnostics: Vec::new(),
                });
            }
            VoicePolicyAdmission::Accepted { stolen_instances } => {
                if !stolen_instances.is_empty() {
                    newengine_ulog_api::ulog::trace!(
                        "audio voice policy: admitted stream group='{}' scope={:?} limit={} steal_rule={:?} stolen_instances={:?}",
                        policy.group,
                        policy.scope,
                        policy.limit,
                        policy.steal_rule,
                        stolen_instances
                    );
                }
            }
        }

        let voice_id = self.alloc_voice_id();
        self.voices.insert(
            voice_id,
            VoiceEntry {
                control: None,
                source: VoiceSource::Stream {
                    uri,
                    buffer: request.buffer,
                    source_duration: metadata.source_duration,
                },
                route: request.route.clone(),
                gain: request.gain,
                speed: 1.0,
                looping: request.looping,
                spatial: request.spatial,
                attenuation: request.attenuation,
                acoustic: request.acoustic,
                propagation: propagation_state(
                    self.listener,
                    self.listener_velocity,
                    request.spatial,
                    [0.0; 3],
                ),
                emitter_velocity: [0.0; 3],
                last_spatial_update: request.spatial.map(|_| Instant::now()),
                environment: request.environment,
                stream_stats: None,
                physical_source_origin: Duration::ZERO,
                concurrency_group: policy.group,
                concurrency_scope: policy.scope,
                concurrency_scope_id: scope_key,
                policy_instance_id,
                voice_budget: policy.budget,
                priority: policy.priority,
                paused: false,
                render_start_sample: request.render_start_sample,
                virtual_source_position: initial_position,
                virtual_since: Some(Instant::now()),
            },
        );
        self.rebalance_physical_voices();

        if request.render_start_sample.is_some()
            && self.voices.get(&voice_id).is_some_and(VoiceEntry::is_virtual)
        {
            let _ = self.remove_voice(voice_id);
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                voice_ids: Vec::new(),
                message: "exact render-scheduled voice requires a physical voice slot".to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        }

        let Some(voice) = self.voices.get(&voice_id) else {
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                voice_ids: Vec::new(),
                message: "logical stream disappeared during provider rebalance".to_owned(),
                virtualized: false,
                diagnostics: Vec::new(),
            });
        };
        Ok(AudioPlayAck {
            accepted: true,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            voice_id: Some(voice_id),
            voice_ids: vec![voice_id],
            message: if voice.is_virtual() {
                "stream accepted as logical virtual voice; awaiting a physical mixer slot".to_owned()
            } else {
                String::new()
            },
            virtualized: voice.is_virtual(),
            diagnostics: Vec::new(),
        })
    }

}

include!("state_playback/cues.rs");

impl AudioRuntimeState {
    fn play_feedback(&mut self, event: AudioFeedbackEvent) -> AudioFeedbackAck {
        self.prune_finished();
        let (frequency, duration_ms) = feedback_tone(&event.id);
        let gain = sanitize_gain(DEFAULT_UI_TONE_GAIN * event.intensity.clamp(0.0, 1.0));
        let voice_id = self.alloc_voice_id();
        let policy_instance_id = self.alloc_policy_instance_id();
        self.voices.insert(
            voice_id,
            VoiceEntry {
                control: None,
                source: VoiceSource::Tone {
                    frequency,
                    duration: Duration::from_millis(duration_ms),
                },
                route: AudioRouteId::default(),
                gain,
                speed: 1.0,
                looping: false,
                spatial: None,
                attenuation: None,
                acoustic: AudioAcousticState::clear(),
                propagation: AudioPropagationState::default(),
                emitter_velocity: [0.0; 3],
                last_spatial_update: None,
                environment: AudioEnvironmentState::clear(),
                stream_stats: None,
                physical_source_origin: Duration::ZERO,
                concurrency_group: String::new(),
                concurrency_scope: AudioConcurrencyScope::Global,
                concurrency_scope_id: None,
                policy_instance_id,
                voice_budget: String::new(),
                priority: UI_FEEDBACK_PRIORITY,
                paused: false,
                render_start_sample: None,
                virtual_source_position: Duration::ZERO,
                virtual_since: Some(Instant::now()),
            },
        );
        self.rebalance_physical_voices();
        AudioFeedbackAck {
            accepted: self.voices.contains_key(&voice_id),
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            queued_events: self.voices.len(),
        }
    }
}
