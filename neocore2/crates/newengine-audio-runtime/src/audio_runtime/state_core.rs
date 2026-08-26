pub struct AudioRuntimeState {
    assets: AssetServiceClient,
    output: Option<MixerDeviceSink>,
    output_rx: Option<mpsc::Receiver<Result<MixerDeviceSink, String>>>,
    output_error: Option<String>,
    output_init_started: bool,
    voices: HashMap<u64, VoiceEntry>,
    next_voice_id: u64,
    cue_counter: u64,
    listener: AudioListenerState,
    bus_gains: BTreeMap<AudioBus, f32>,
    clips: HashMap<String, CachedClip>,
    cues: HashMap<String, SoundCue>,
    cue_layers: HashMap<String, Vec<YscdRuntimeLayer>>,
    cue_meta: HashMap<String, YscdRuntimeMeta>,
    embedded_yscd_clips: HashMap<String, EmbeddedYscdClipLocator>,
    materialization_errors: HashMap<u64, String>,
    cached_bytes: usize,
    cache_limit_bytes: usize,
    max_physical_voices: usize,
}

impl AudioRuntimeState {
    /// Creates only the semantic/provider state. Physical device initialization is lazy and
    /// never starts from plugin/DLL init; Windows audio APIs may load COM/MMDevAPI modules and
    /// are not safe to initialize under the plugin loader lifecycle.
    pub fn open_default(assets: AssetServiceClient) -> Result<Self, String> {
        let mut bus_gains = BTreeMap::new();
        for bus in AudioBus::all() {
            bus_gains.insert(bus, 1.0);
        }
        Ok(Self {
            assets,
            output: None,
            output_rx: None,
            output_error: None,
            output_init_started: false,
            voices: HashMap::new(),
            next_voice_id: 1,
            cue_counter: 1,
            listener: AudioListenerState::default(),
            bus_gains,
            clips: HashMap::new(),
            cues: HashMap::new(),
            cue_layers: HashMap::new(),
            cue_meta: HashMap::new(),
            embedded_yscd_clips: HashMap::new(),
            materialization_errors: HashMap::new(),
            cached_bytes: 0,
            cache_limit_bytes: cache_limit_bytes_from_env(),
            max_physical_voices: max_physical_voices_from_env(),
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
                self.output = Some(output);
                self.output_rx = None;
                self.output_error = None;
                newengine_ulog_api::ulog::info!(
                    "audio device init: phase='ready' provider='{}'",
                    NATIVE_AUDIO_PROVIDER_ROUTE
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

    #[inline]
    fn alloc_voice_id(&mut self) -> u64 {
        let id = self.next_voice_id.max(1);
        self.next_voice_id = id.wrapping_add(1).max(1);
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
    fn bus_gain(&self, bus: AudioBus) -> f32 {
        let master = self
            .bus_gains
            .get(&AudioBus::Master)
            .copied()
            .unwrap_or(1.0);
        if bus == AudioBus::Master {
            master
        } else {
            master * self.bus_gains.get(&bus).copied().unwrap_or(1.0)
        }
    }

    #[inline]
    fn voice_audibility(&self, voice: &VoiceEntry) -> f32 {
        sanitize_gain(voice.gain)
            * self.bus_gain(voice.bus)
            * voice.attenuation_gain(self.listener)
            * voice.acoustic.sanitized().transmission_gain
    }

    fn refresh_voice_gains(&self) {
        for voice in self.voices.values() {
            if let Some(control) = voice.control.as_ref() {
                control.set_volume(self.voice_audibility(voice));
            }
        }
    }
}
