pub struct AudioRuntimeState {
    assets: AssetServiceClient,
    output: Option<MixerDeviceSink>,
    render_graph: Option<NativeBlockRenderGraphHandle>,
    output_rx: Option<mpsc::Receiver<Result<MixerDeviceSink, String>>>,
    output_error: Option<String>,
    output_init_started: bool,
    voices: HashMap<u64, VoiceEntry>,
    stream_metadata: HashMap<String, StreamSourceMetadata>,
    stream_promotions: u64,
    stream_demotions: u64,
    next_voice_id: u64,
    next_policy_instance_id: u64,
    cue_counter: u64,
    listener: AudioListenerState,
    listener_velocity: [f32; 3],
    listener_updated_at: Option<Instant>,
    route_gains: BTreeMap<AudioRouteId, f32>,
    clips: HashMap<String, CachedClip>,
    cues: HashMap<String, SoundCue>,
    ysncd_dictionaries: HashMap<String, Arc<newengine_asset_format_nef8::YsncdDictionary>>,
    cue_layers: HashMap<String, Vec<YsncdRuntimeLayer>>,
    cue_clips_by_name: HashMap<String, HashMap<String, SoundCueClip>>,
    cue_sound_graphs: HashMap<String, Arc<newengine_asset_format_nef8::YsncdSoundGraph>>,
    sound_graph_sequences: HashMap<String, u64>,
    cue_meta: HashMap<String, YsncdRuntimeMeta>,
    cue_history: HashMap<String, VecDeque<String>>,
    embedded_ysncd_clips: HashMap<String, EmbeddedYsncdClipLocator>,
    materialization_errors: HashMap<u64, String>,
    cached_bytes: usize,
    cache_limit_bytes: usize,
    max_physical_voices: usize,
    voice_budget_reservations: BTreeMap<String, usize>,
    room_buses: SharedRoomLateBusManager,
}

impl AudioRuntimeState {
    /// Creates only the semantic/provider state. Physical device initialization is lazy and
    /// never starts from plugin/DLL init; Windows audio APIs may load COM/MMDevAPI modules and
    /// are not safe to initialize under the plugin loader lifecycle.
    pub fn open_default(assets: AssetServiceClient) -> Result<Self, String> {
        Ok(Self {
            assets,
            output: None,
            render_graph: None,
            output_rx: None,
            output_error: None,
            output_init_started: false,
            voices: HashMap::new(),
            stream_metadata: HashMap::new(),
            stream_promotions: 0,
            stream_demotions: 0,
            next_voice_id: 1,
            next_policy_instance_id: 1,
            cue_counter: 1,
            listener: AudioListenerState::default(),
            listener_velocity: [0.0; 3],
            listener_updated_at: None,
            route_gains: BTreeMap::new(),
            clips: HashMap::new(),
            cues: HashMap::new(),
            ysncd_dictionaries: HashMap::new(),
            cue_layers: HashMap::new(),
            cue_clips_by_name: HashMap::new(),
            cue_sound_graphs: HashMap::new(),
            sound_graph_sequences: HashMap::new(),
            cue_meta: HashMap::new(),
            cue_history: HashMap::new(),
            embedded_ysncd_clips: HashMap::new(),
            materialization_errors: HashMap::new(),
            cached_bytes: 0,
            cache_limit_bytes: cache_limit_bytes_from_env(),
            max_physical_voices: max_physical_voices_from_env(),
            voice_budget_reservations: BTreeMap::new(),
            room_buses: SharedRoomLateBusManager::new(),
        })
    }

    fn start_output_init(&mut self) {
        if self.output.is_some() || self.output_init_started || self.output_error.is_some() {
            return;
        }
        self.output_init_started = true;
        let (output_tx, output_rx) = mpsc::channel();
        match thread::Builder::new()
            .name("engine.audio.device-init".to_owned())
            .spawn(move || {
                newengine_ulog_api::ulog::info!(
                    "audio device init: phase='begin' thread='engine.audio.device-init' trigger='first-play'"
                );
                let result = DeviceSinkBuilder::open_default_sink()
                    .map_err(|error| format!("open default audio output failed: {error}"))
                    .map(|mut output| {
                        output.log_on_drop(false);
                        output
                    });
                let _ = output_tx.send(result);
            })
        {
            Ok(_) => {
                self.output_rx = Some(output_rx);
            }
            Err(error) => {
                self.output_error = Some(format!("spawn audio device init worker failed: {error}"));
            }
        }
    }

    fn poll_output_init(&mut self) {
        if self.output.is_some() {
            return;
        }
        let Some(receiver) = self.output_rx.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(output)) => {
                let channels = output.config().channel_count();
                let sample_rate = output.config().sample_rate();
                let (render_graph, master_source) = native_block_render_graph(channels, sample_rate);
                output.mixer().add(master_source);
                self.render_graph = Some(render_graph);
                self.output = Some(output);
                self.output_rx = None;
                self.output_error = None;
                newengine_ulog_api::ulog::info!(
                    "audio device init: phase='ready' provider='{}' executor='native-block-render' sample_rate={} channels={} block_frames={}",
                    NATIVE_AUDIO_PROVIDER_ROUTE,
                    sample_rate.get(),
                    channels.get(),
                    block_render::NATIVE_BLOCK_FRAMES,
                );
            }
            Ok(Err(error)) => {
                self.output_rx = None;
                self.output_error = Some(error.clone());
                newengine_ulog_api::ulog::warn!(
                    "audio device init: phase='failed' provider='{}' err='{}'",
                    NATIVE_AUDIO_PROVIDER_ROUTE,
                    error
                );
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.output_rx = None;
                self.output_error = Some("audio device init worker disconnected".to_owned());
                newengine_ulog_api::ulog::warn!(
                    "audio device init: phase='failed' provider='{}' err='worker disconnected'",
                    NATIVE_AUDIO_PROVIDER_ROUTE
                );
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    #[inline]
    fn output_ready(&mut self) -> bool {
        self.poll_output_init();
        self.output.is_some()
    }

    fn render_clock(&mut self) -> AudioRenderClock {
        self.start_output_init();
        self.poll_output_init();
        let Some(g) = self.render_graph.as_ref() else { return AudioRenderClock { ready:false, sample_rate:0, sample:0, block_frames:block_render::NATIVE_BLOCK_FRAMES as u32 }; };
        AudioRenderClock { ready:true, sample_rate:g.sample_rate().get(), sample:g.output_sample(), block_frames:block_render::NATIVE_BLOCK_FRAMES as u32 }
    }

    fn validate_render_start_sample(&mut self, at: Option<u64>) -> Result<(), String> {
        let Some(at)=at else { return Ok(()); };
        let c=self.render_clock();
        if !c.ready { return Err("native render clock is not ready for exact scheduled onset".to_owned()); }
        if at<c.sample { return Err(format!("scheduled render start sample {at} is in the past; provider sample={}",c.sample)); }
        Ok(())
    }

    fn schedule_voice_render(
        &mut self,
        request: AudioVoiceRenderScheduleRequest,
    ) -> AudioVoiceRenderScheduleAck {
        self.poll_output_init();
        let provider = NATIVE_AUDIO_PROVIDER_ROUTE.to_owned();
        let reject = |message: String| AudioVoiceRenderScheduleAck {
            accepted: false,
            voice_id: request.voice_id,
            at_sample: request.at_sample,
            schedule_id: request.schedule_id,
            provider: provider.clone(),
            message,
        };

        let Some(graph) = self.render_graph.as_ref() else {
            return reject("native render clock is not ready".to_owned());
        };
        if request.schedule_id == 0 {
            return reject("render schedule_id must be non-zero".to_owned());
        }

        let Some(control) = self
            .voices
            .get(&request.voice_id)
            .and_then(|voice| voice.control.as_ref())
        else {
            return reject("voice is not physically materialized".to_owned());
        };
        let render = match control {
            VoiceControl::Flat { render, .. } | VoiceControl::Spatial { render, .. } => render,
        };

        let result = match request.action {
            AudioVoiceRenderAction::Cancel => render.cancel_scheduled(request.schedule_id),
            AudioVoiceRenderAction::GainRamp {
                target_gain,
                duration_samples,
            } => {
                if request.at_sample < graph.output_sample() {
                    Err(format!(
                        "render schedule sample {} is in the past; provider sample={}",
                        request.at_sample,
                        graph.output_sample()
                    ))
                } else if !target_gain.is_finite() {
                    Err("scheduled render gain must be finite".to_owned())
                } else {
                    let target_output_gain = self
                        .voices
                        .get(&request.voice_id)
                        .map(|voice| {
                            sanitize_gain(target_gain)
                                * self.route_gain(&voice.route)
                                * voice.attenuation_gain(self.listener)
                                * voice.propagated_acoustic().transmission_gain
                        })
                        .unwrap_or_else(|| sanitize_gain(target_gain));
                    render.schedule_gain_at(
                        request.at_sample,
                        target_output_gain,
                        duration_samples,
                        request.schedule_id,
                    )
                }
            }
            AudioVoiceRenderAction::Stop => {
                if request.at_sample < graph.output_sample() {
                    Err(format!(
                        "render schedule sample {} is in the past; provider sample={}",
                        request.at_sample,
                        graph.output_sample()
                    ))
                } else {
                    render.schedule_stop_at(request.at_sample, request.schedule_id)
                }
            }
        };

        match result {
            Ok(()) => AudioVoiceRenderScheduleAck {
                accepted: true,
                voice_id: request.voice_id,
                at_sample: request.at_sample,
                schedule_id: request.schedule_id,
                provider,
                message: String::new(),
            },
            Err(message) => reject(message),
        }
    }

    #[inline]
    fn alloc_voice_id(&mut self) -> u64 {
        let id = self.next_voice_id.max(1);
        self.next_voice_id = id.wrapping_add(1).max(1);
        id
    }

    #[inline]
    fn alloc_policy_instance_id(&mut self) -> u64 {
        let id = self.next_policy_instance_id.max(1);
        self.next_policy_instance_id = id.wrapping_add(1).max(1);
        id
    }

    fn remove_voice(&mut self, voice_id: u64) -> Option<VoiceEntry> {
        self.materialization_errors.remove(&voice_id);
        let voice = self.voices.remove(&voice_id)?;
        if let Some(control) = voice.control.as_ref() {
            control.stop();
        }
        Some(voice)
    }

    fn prune_finished(&mut self) {
        let now = Instant::now();
        let finished = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| voice.is_finished(now).then_some(*voice_id))
            .collect::<Vec<_>>();
        for voice_id in finished {
            let _ = self.remove_voice(voice_id);
        }
    }

    #[inline]
    fn route_is_configured(&self, route: &AudioRouteId) -> bool {
        route.0.is_empty() || self.route_gains.contains_key(route)
    }

    #[inline]
    fn route_gain(&self, route: &AudioRouteId) -> f32 {
        if route.0.is_empty() {
            1.0
        } else {
            self.route_gains.get(route).copied().unwrap_or(1.0)
        }
    }

    #[inline]
    fn voice_output_gain(&self, voice: &VoiceEntry) -> f32 {
        sanitize_gain(voice.gain)
            * self.route_gain(&voice.route)
            * voice.attenuation_gain(self.listener)
            * voice.propagated_acoustic().transmission_gain
    }

    #[inline]
    fn voice_audibility(&self, voice: &VoiceEntry) -> f32 {
        self.voice_output_gain(voice) * voice.environment_audibility_gain()
    }

    fn refresh_voice_gains(&self) {
        for voice in self.voices.values() {
            if let Some(control) = voice.control.as_ref() {
                control.set_volume(self.voice_output_gain(voice));
            }
        }
    }
}
