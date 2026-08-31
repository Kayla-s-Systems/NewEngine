impl AudioRuntimeState {
    fn play_clip(&mut self, request: AudioPlayRequest) -> Result<AudioPlayAck, String> {
        let policy_instance_id = self.alloc_policy_instance_id();
        self.play_clip_with_policy(
            request,
            AudioVoicePolicy::default(),
            None,
            policy_instance_id,
        )
    }

    fn play_clip_with_policy(
        &mut self,
        request: AudioPlayRequest,
        policy: AudioVoicePolicy,
        scope_id: Option<u64>,
        policy_instance_id: u64,
    ) -> Result<AudioPlayAck, String> {
        self.prune_finished();
        let request = request.sanitized();
        let uri = normalize_vfs_path(&request.clip.uri)?;
        let source_duration = self.clip_source_duration(&uri)?;
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
                bus: request.bus,
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
                virtual_source_position: Duration::ZERO,
                virtual_since: Some(now),
            },
        );
        self.rebalance_physical_voices();

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
                bus: request.bus,
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
                virtual_source_position: initial_position,
                virtual_since: Some(Instant::now()),
            },
        );
        self.rebalance_physical_voices();

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

    fn load_cue(&mut self, cue_reference: &str) -> Result<SoundCue, String> {
        let reference = newengine_assets_api::parse_asset_reference(cue_reference)
            .map_err(|error| format!("audio cue reference invalid '{cue_reference}': {error}"))?;
        if !reference.has_extension(newengine_asset_format_nef8::yscd::EXTENSION) {
            return Err(format!(
                "authored SoundCue JSON is retired; cue '{}' must use .yscd@entry",
                reference.canonical
            ));
        }
        reference.require_entry()?;
        let canonical = reference.canonical.clone();
        if let Some(cue) = self.cues.get(&canonical) {
            return Ok(cue.clone());
        }

        let dictionary =
            if let Some(dictionary) = self.yscd_dictionaries.get(&reference.logical_path) {
                Arc::clone(dictionary)
            } else {
                let source =
                    self.assets
                        .raw_bytes_v1(&reference.logical_path)
                        .map_err(|error| {
                            format!(
                                "YSCD VFS read failed logical_path='{}': {error}",
                                reference.logical_path
                            )
                        })?;
                let decoded = Arc::new(newengine_asset_format_nef8::decode_yscd_nef8(
                    &source,
                    &reference.logical_path,
                )?);
                newengine_ulog_api::ulog::info!(
                    "YSCD dictionary cache miss path='{}' cues={} bytes={} policy='decode-once'",
                    reference.logical_path,
                    decoded.cues.len(),
                    source.len(),
                );
                self.yscd_dictionaries
                    .insert(reference.logical_path.clone(), Arc::clone(&decoded));
                decoded
            };
        let cue_name = reference.entry.as_deref().expect("entry required above");
        let authored = dictionary.cue(cue_name).ok_or_else(|| {
            format!(
                "YSCD cue '{}' not found in '{}'",
                cue_name, reference.logical_path
            )
        })?;

        let mut clips = Vec::with_capacity(authored.clips.len());
        let mut clips_by_name = HashMap::<String, SoundCueClip>::new();
        for (clip_index, clip) in authored.clips.iter().enumerate() {
            let key = embedded_yscd_clip_key(&canonical, clip_index, &clip.codec);
            self.embedded_yscd_clips.insert(
                key.clone(),
                EmbeddedYscdClipLocator {
                    dictionary_path: reference.logical_path.clone(),
                    cue_name: authored.name.clone(),
                    clip_index,
                },
            );
            if !self.clips.contains_key(&key) {
                let _ = self.cache_clip_bytes(key.clone(), clip.bytes.clone())?;
            }
            let runtime_clip = SoundCueClip {
                clip: newengine_audio_api::AudioClipRef::new(key),
                weight: clip.weight,
                gain: clip.gain,
                pitch: clip.pitch,
            };
            clips_by_name.insert(clip.name.trim().to_ascii_lowercase(), runtime_clip.clone());
            clips.push(runtime_clip);
        }

        let mut runtime_layers = Vec::with_capacity(authored.descriptor.layers.len());
        for layer in &authored.descriptor.layers {
            let mut layer_clips = Vec::with_capacity(layer.clip_names.len());
            for clip_name in &layer.clip_names {
                let key = clip_name.trim().to_ascii_lowercase();
                let clip = clips_by_name.get(&key).cloned().ok_or_else(|| {
                    format!(
                        "YSCD cue '{}' layer '{}' references unknown clip '{}'",
                        authored.name, layer.name, clip_name
                    )
                })?;
                layer_clips.push(clip);
            }
            if layer_clips.is_empty() {
                return Err(format!(
                    "YSCD cue '{}' layer '{}' resolved no clips",
                    authored.name, layer.name
                ));
            }
            runtime_layers.push(YscdRuntimeLayer {
                name: layer.name.trim().to_owned(),
                role: layer.role.trim().to_ascii_lowercase(),
                clips: layer_clips,
                gain: sanitize_gain(layer.gain),
                pitch: sanitize_speed(layer.pitch),
                attenuation: layer
                    .attenuation
                    .as_ref()
                    .map(audio_attenuation_from_yscd)
                    .transpose()?,
            });
        }

        let embedded_bytes = authored
            .clips
            .iter()
            .map(|clip| clip.bytes.len())
            .sum::<usize>();
        newengine_ulog_api::ulog::info!(
            "YSCD resolve dictionary='{}' cue='{}' embedded_clip_bytes={} clips={} layers={} sound_graph_nodes={} source='engine.assets.raw_bytes_v1' body='NEF8/YSCD-v1'",
            reference.logical_path,
            authored.name,
            embedded_bytes,
            authored.clips.len(),
            runtime_layers.len(),
            authored
                .descriptor
                .sound_graph
                .as_ref()
                .map_or(0, |graph| graph.nodes.len()),
        );

        let cue = SoundCue {
            version: 1,
            clips,
            gain_range: authored.descriptor.gain_range,
            pitch_range: authored.descriptor.pitch_range,
            bus: audio_bus_from_yscd(&authored.descriptor.bus)?,
            looping: authored.descriptor.looping,
            concurrency_group: authored.descriptor.concurrency_group.clone(),
            concurrency_limit: authored.descriptor.concurrency_limit,
            concurrency_scope: concurrency_scope_from_yscd(
                &authored.descriptor.concurrency_scope,
            )?,
            steal_rule: voice_steal_rule_from_yscd(&authored.descriptor.steal_rule)?,
            voice_budget: authored.descriptor.voice_budget.clone(),
            priority: authored.descriptor.priority,
            repeat_avoidance: authored.descriptor.repeat_avoidance,
            spatial_policy: sound_cue_spatial_policy_from_yscd(
                &authored.descriptor.spatial_policy,
            )?,
            attenuation: authored
                .descriptor
                .attenuation
                .as_ref()
                .map(audio_attenuation_from_yscd)
                .transpose()?,
        }
        .sanitized()?;
        self.cue_layers.insert(canonical.clone(), runtime_layers);
        self.cue_clips_by_name
            .insert(canonical.clone(), clips_by_name);
        if let Some(graph) = authored.descriptor.sound_graph.as_ref() {
            self.cue_sound_graphs
                .insert(canonical.clone(), Arc::new(graph.clone()));
        } else {
            self.cue_sound_graphs.remove(&canonical);
        }
        self.cue_meta.insert(
            canonical.clone(),
            YscdRuntimeMeta {
                dictionary_path: reference.logical_path.clone(),
                cue_name: authored.name.clone(),
                embedded_bytes,
            },
        );
        self.cues.insert(canonical, cue.clone());
        Ok(cue)
    }

    fn preload_cue(&mut self, request: AudioCuePreloadRequest) -> Result<AudioPreloadAck, String> {
        let parsed = newengine_assets_api::parse_asset_reference(&request.cue.logical_path)
            .map_err(|error| {
                format!(
                    "audio cue reference invalid '{}': {error}",
                    request.cue.logical_path
                )
            })?;
        let canonical = parsed.canonical.clone();
        let cue = self.load_cue(&request.cue.logical_path)?;
        let clip_count = cue.clips.len();
        let layer_count = self.cue_layers.get(&canonical).map_or(0, Vec::len);
        let graph_node_count = self
            .cue_sound_graphs
            .get(&canonical)
            .map_or(0, |graph| graph.nodes.len());
        let mut bytes = 0usize;
        let mut all_cached = true;
        for entry in cue.clips {
            let ack = self.preload(AudioPreloadRequest { clip: entry.clip })?;
            bytes = bytes.saturating_add(ack.bytes);
            all_cached &= ack.cached;
        }

        // Device creation remains forbidden during DLL/plugin initialization, but cue
        // preload runs on the normal runtime loading path. Starting the async worker
        // here hides first-shot device latency without blocking the loading thread.
        self.start_output_init();
        self.poll_output_init();

        let mut diagnostics = self
            .cue_meta
            .get(&canonical)
            .map(|meta| {
                vec![format!(
                    "YSCD resolve dictionary='{}' cue='{}' embedded_clip_bytes={} clips={} layers={} sound_graph_nodes={}",
                    meta.dictionary_path,
                    meta.cue_name,
                    meta.embedded_bytes,
                    clip_count,
                    layer_count,
                    graph_node_count,
                )]
            })
            .unwrap_or_default();
        let device_state = if self.output.is_some() {
            "ready"
        } else if self.output_error.is_some() {
            "failed"
        } else if self.output_init_started {
            "initializing"
        } else {
            "idle"
        };
        diagnostics.push(format!(
            "audio device prewarm state='{}' init_started={} output_ready={} error='{}'",
            device_state,
            self.output_init_started,
            self.output.is_some(),
            self.output_error.as_deref().unwrap_or(""),
        ));

        Ok(AudioPreloadAck {
            accepted: true,
            cached: all_cached,
            bytes,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            diagnostics,
        })
    }

    fn play_cue(&mut self, request: AudioCuePlayRequest) -> Result<AudioPlayAck, String> {
        let request = request.sanitized();
        if request.version != 1 {
            return Err(format!(
                "unsupported AudioCuePlayRequest version {}",
                request.version
            ));
        }
        let parsed = newengine_assets_api::parse_asset_reference(&request.cue.logical_path)
            .map_err(|error| {
                format!(
                    "audio cue reference invalid '{}': {error}",
                    request.cue.logical_path
                )
            })?;
        let canonical = parsed.canonical.clone();
        let cue = self.load_cue(&request.cue.logical_path)?;
        let layers = self.cue_layers.get(&canonical).cloned().unwrap_or_default();
        let voice_policy = cue.voice_policy().sanitized()?;
        let policy_instance_id = self.alloc_policy_instance_id();
        let seed = request.seed.unwrap_or_else(|| {
            let seed = self.cue_counter;
            self.cue_counter = self.cue_counter.wrapping_add(1).max(1);
            seed
        }) ^ stable_text_hash(&request.cue.logical_path);
        let spatial = match cue.spatial_policy {
            SoundCueSpatialPolicy::NonSpatial => None,
            SoundCueSpatialPolicy::Spatial => Some(AudioSpatialParams {
                position: request.position.ok_or_else(|| {
                    format!(
                        "SoundCue '{}' requires a spatial position",
                        request.cue.logical_path
                    )
                })?,
            }),
            SoundCueSpatialPolicy::Inherit => request
                .position
                .map(|position| AudioSpatialParams { position }),
        };

        if let Some(graph) = self.cue_sound_graphs.get(&canonical).cloned() {
            let plans = self.evaluate_sound_graph(
                &canonical,
                &graph,
                &request.parameters,
                seed,
                request.scope_id,
            )?;
            let clips_by_name = self
                .cue_clips_by_name
                .get(&canonical)
                .cloned()
                .ok_or_else(|| format!("YSCD SoundGraph '{}' clip map is unavailable", canonical))?;
            let mut primary: Option<AudioPlayAck> = None;
            let mut accepted_voice_ids = Vec::<u64>::new();
            let mut diagnostics = Vec::<String>::new();
            let mut rejection_reason: Option<String> = None;
            for (index, plan) in plans.iter().enumerate() {
                let selected = clips_by_name.get(&plan.clip_name).cloned().ok_or_else(|| {
                    format!(
                        "YSCD SoundGraph '{}' resolved unknown runtime clip '{}'",
                        canonical, plan.clip_name
                    )
                })?;
                let random_a = splitmix64(
                    seed ^ stable_text_hash(&plan.label) ^ (index as u64).rotate_left(17),
                );
                let random_b = splitmix64(random_a);
                let gain = sanitize_gain(
                    request.gain
                        * plan.gain
                        * selected.gain
                        * sample_range(cue.gain_range, unit_f32(random_a)),
                );
                let speed = sanitize_speed(
                    request.pitch
                        * plan.pitch
                        * selected.pitch
                        * sample_range(cue.pitch_range, unit_f32(random_b)),
                );
                let ack = self.play_clip_with_policy(
                    AudioPlayRequest {
                        version: 1,
                        clip: selected.clip.clone(),
                        bus: cue.bus,
                        gain,
                        speed,
                        looping: cue.looping,
                        spatial,
                        attenuation: cue.attenuation.clone(),
                        acoustic: request.acoustic,
                        environment: request.environment,
                    },
                    voice_policy.clone(),
                    request.scope_id,
                    policy_instance_id,
                )?;
                if let Some(diagnostic) =
                    self.yscd_play_diagnostic(&canonical, &format!("graph:{}", plan.label), &selected, &ack)
                {
                    diagnostics.push(diagnostic);
                }
                if ack.accepted {
                    accepted_voice_ids.extend(ack.voice_ids.iter().copied());
                    if primary.is_none() {
                        primary = Some(ack.clone());
                    }
                } else if rejection_reason.is_none() {
                    rejection_reason = Some(ack.message.clone());
                }
            }
            if let Some(mut ack) = primary {
                accepted_voice_ids.sort_unstable();
                accepted_voice_ids.dedup();
                ack.voice_ids = accepted_voice_ids;
                ack.message = format!(
                    "YSCD SoundGraph accepted logical_voices={}",
                    ack.voice_ids.len()
                );
                ack.diagnostics = diagnostics;
                return Ok(ack);
            }
            return Ok(AudioPlayAck {
                accepted: false,
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                voice_id: None,
                voice_ids: Vec::new(),
                message: rejection_reason.unwrap_or_else(|| {
                    "YSCD SoundGraph produced no accepted logical voices".to_owned()
                }),
                virtualized: false,
                diagnostics,
            });
        }

        if layers.is_empty() {
            let random_a = splitmix64(seed);
            let random_b = splitmix64(random_a);
            let random_c = splitmix64(random_b);
            let recent = self
                .cue_history
                .get(&canonical)
                .cloned()
                .unwrap_or_default();
            let selected = select_weighted_clips_avoiding(&cue.clips, unit_f32(random_a), &recent)
                .cloned()
                .ok_or_else(|| "SoundCue weighted selection produced no clip".to_owned())?;
            let gain = sanitize_gain(
                request.gain * selected.gain * sample_range(cue.gain_range, unit_f32(random_b)),
            );
            let speed = sanitize_speed(
                request.pitch * selected.pitch * sample_range(cue.pitch_range, unit_f32(random_c)),
            );
            let ack = self.play_clip_with_policy(
                AudioPlayRequest {
                    version: 1,
                    clip: selected.clip.clone(),
                    bus: cue.bus,
                    gain,
                    speed,
                    looping: cue.looping,
                    spatial,
                    attenuation: cue.attenuation.clone(),
                    acoustic: request.acoustic,
                    environment: request.environment,
                },
                voice_policy.clone(),
                request.scope_id,
                policy_instance_id,
            )?;
            let mut ack = ack;
            if ack.accepted {
                self.remember_cue_selection(&canonical, &selected.clip.uri, cue.repeat_avoidance);
            }
            if let Some(diagnostic) = self.yscd_play_diagnostic(&canonical, "body", &selected, &ack)
            {
                ack.diagnostics.push(diagnostic);
            }
            return Ok(ack);
        }

        let mut primary: Option<AudioPlayAck> = None;
        let mut rejection_reason: Option<String> = None;
        let mut accepted_layers = 0usize;
        let mut accepted_voice_ids = Vec::<u64>::new();
        let mut diagnostics = Vec::with_capacity(layers.len());
        for (index, layer) in layers.iter().enumerate() {
            let layer_seed = splitmix64(seed ^ stable_text_hash(&layer.name) ^ index as u64);
            let random_a = splitmix64(layer_seed);
            let random_b = splitmix64(random_a);
            let random_c = splitmix64(random_b);
            let history_key = format!("{canonical}#{}", layer.name);
            let recent = self
                .cue_history
                .get(&history_key)
                .cloned()
                .unwrap_or_default();
            let selected =
                select_weighted_clips_avoiding(&layer.clips, unit_f32(random_a), &recent)
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "YSCD layer '{}' weighted selection produced no clip",
                            layer.name
                        )
                    })?;
            let gain = sanitize_gain(
                request.gain
                    * layer.gain
                    * selected.gain
                    * sample_range(cue.gain_range, unit_f32(random_b)),
            );
            let speed = sanitize_speed(
                request.pitch
                    * layer.pitch
                    * selected.pitch
                    * sample_range(cue.pitch_range, unit_f32(random_c)),
            );
            let ack = self.play_clip_with_policy(
                AudioPlayRequest {
                    version: 1,
                    clip: selected.clip.clone(),
                    bus: cue.bus,
                    gain,
                    speed,
                    looping: cue.looping,
                    spatial,
                    attenuation: layer
                        .attenuation
                        .clone()
                        .or_else(|| cue.attenuation.clone()),
                    acoustic: request.acoustic,
                    environment: request.environment,
                },
                voice_policy.clone(),
                request.scope_id,
                policy_instance_id,
            )?;
            if let Some(diagnostic) =
                self.yscd_play_diagnostic(&canonical, &layer.name, &selected, &ack)
            {
                diagnostics.push(diagnostic);
            }
            if !ack.accepted && rejection_reason.is_none() && !ack.message.is_empty() {
                rejection_reason = Some(ack.message.clone());
            }
            if ack.accepted {
                accepted_voice_ids.extend(ack.voice_ids.iter().copied());
                self.remember_cue_selection(&history_key, &selected.clip.uri, cue.repeat_avoidance);
                accepted_layers = accepted_layers.saturating_add(1);
                let preferred_primary = matches!(layer.role.as_str(), "body" | "near");
                if primary.is_none() || preferred_primary {
                    primary = Some(ack.clone());
                }
            }
        }

        if let Some(mut ack) = primary {
            accepted_voice_ids.sort_unstable();
            accepted_voice_ids.dedup();
            ack.voice_ids = accepted_voice_ids;
            ack.message = format!("YSCD layered cue accepted layers={accepted_layers}");
            ack.diagnostics = diagnostics;
            return Ok(ack);
        }
        Ok(AudioPlayAck {
            accepted: false,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            voice_id: None,
            voice_ids: Vec::new(),
            message: rejection_reason
                .unwrap_or_else(|| "YSCD layered cue produced no accepted voices".to_owned()),
            virtualized: false,
            diagnostics,
        })
    }

    fn remember_cue_selection(&mut self, key: &str, clip_uri: &str, limit: usize) {
        if limit == 0 {
            return;
        }
        let history = self.cue_history.entry(key.to_owned()).or_default();
        history.retain(|entry| entry != clip_uri);
        history.push_back(clip_uri.to_owned());
        while history.len() > limit {
            history.pop_front();
        }
    }

    fn yscd_play_diagnostic(
        &self,
        canonical: &str,
        layer: &str,
        selected: &SoundCueClip,
        ack: &AudioPlayAck,
    ) -> Option<String> {
        let meta = self.cue_meta.get(canonical)?;
        let clip_bytes = self
            .clips
            .get(&selected.clip.uri)
            .map(CachedClip::len)
            .unwrap_or(0);
        let voice = ack.voice_id.and_then(|voice_id| self.voices.get(&voice_id));
        let physical_voice = voice.is_some_and(VoiceEntry::is_physical);
        let arbiter_selected = ack
            .voice_id
            .is_some_and(|voice_id| self.desired_physical_voices().contains(&voice_id));
        let audibility = voice
            .map(|voice| self.voice_audibility(voice))
            .unwrap_or(0.0);
        let distance = voice
            .map(|voice| voice.distance_to(self.listener))
            .unwrap_or(0.0);
        let attenuation_gain = voice
            .map(|voice| voice.attenuation_gain(self.listener))
            .unwrap_or(0.0);
        let bus_gain = voice.map(|voice| self.bus_gain(voice.bus)).unwrap_or(0.0);
        let transmission_gain = voice
            .map(|voice| voice.propagated_acoustic().transmission_gain)
            .unwrap_or(0.0);
        let doppler_ratio = voice
            .map(|voice| voice.propagation.doppler_ratio)
            .unwrap_or(1.0);
        let air_hf_gain = voice
            .map(|voice| voice.propagation.air_high_frequency_gain)
            .unwrap_or(1.0);
        let air_low_pass_hz = voice
            .map(|voice| voice.propagation.air_low_pass_hz)
            .unwrap_or(20_000.0);
        let output_state = if self.output.is_some() {
            "ready"
        } else if self.output_error.is_some() {
            "failed"
        } else if self.output_init_started {
            "initializing"
        } else {
            "idle"
        };
        let materialize_error = ack
            .voice_id
            .and_then(|voice_id| self.materialization_errors.get(&voice_id))
            .map(String::as_str)
            .unwrap_or("");
        Some(format!(
            "YSCD play dictionary='{}' cue='{}' layer='{}' embedded_clip_bytes={} dictionary_embedded_bytes={} physical_voice={} virtualized={} voice_id={:?} output_state='{}' arbiter_selected={} audibility={:.6} distance={:.3} attenuation_gain={:.6} bus_gain={:.3} transmission_gain={:.3} doppler={:.4} air_hf_gain={:.3} air_low_pass_hz={:.0} max_physical_voices={} output_error='{}' materialize_error='{}'",
            meta.dictionary_path,
            meta.cue_name,
            layer,
            clip_bytes,
            meta.embedded_bytes,
            physical_voice,
            ack.virtualized,
            ack.voice_id,
            output_state,
            arbiter_selected,
            audibility,
            distance,
            attenuation_gain,
            bus_gain,
            transmission_gain,
            doppler_ratio,
            air_hf_gain,
            air_low_pass_hz,
            self.max_physical_voices,
            self.output_error.as_deref().unwrap_or(""),
            materialize_error,
        ))
    }

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
                bus: AudioBus::Ui,
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
