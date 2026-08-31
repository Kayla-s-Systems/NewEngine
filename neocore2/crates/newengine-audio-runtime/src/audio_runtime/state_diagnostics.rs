impl AudioRuntimeState {
    fn diagnostics(&mut self) -> AudioDiagnostics {
        let output_ready = self.output_ready();
        self.rebalance_physical_voices();
        let physical_voices = self
            .voices
            .values()
            .filter(|voice| voice.is_physical())
            .count();
        AudioDiagnostics {
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            output_ready,
            active_voices: self.voices.len(),
            spatial_voices: self
                .voices
                .values()
                .filter(|voice| voice.spatial.is_some())
                .count(),
            physical_voices,
            virtual_voices: self.voices.len().saturating_sub(physical_voices),
            max_physical_voices: self.max_physical_voices,
            voice_budget_reservations: self.voice_budget_reservations.clone(),
            attenuated_voices: self
                .voices
                .values()
                .filter(|voice| voice.attenuation.is_some())
                .count(),
            obstructed_voices: self
                .voices
                .values()
                .filter(|voice| voice.acoustic.obstruction > 1.0e-3)
                .count(),
            occluded_voices: self
                .voices
                .values()
                .filter(|voice| voice.acoustic.occlusion > 0.5)
                .count(),
            spectrally_filtered_voices: self
                .voices
                .values()
                .filter(|voice| {
                    let acoustic = voice.propagated_acoustic();
                    acoustic.high_frequency_gain < 0.999 || acoustic.low_pass_hz < 19_999.0
                })
                .count(),
            air_filtered_voices: self
                .voices
                .values()
                .filter(|voice| voice.propagation.air_high_frequency_gain < 0.999)
                .count(),
            doppler_shifted_voices: self
                .voices
                .values()
                .filter(|voice| (voice.propagation.doppler_ratio - 1.0).abs() > 0.002)
                .count(),
            portal_attenuated_voices: self
                .voices
                .values()
                .filter(|voice| voice.environment.portal_gain < 0.999)
                .count(),
            reverberant_voices: self
                .voices
                .values()
                .filter(|voice| voice.environment.is_wet())
                .count(),
            active_room_buses: self.room_buses.active_bus_count(),
            max_room_buses: MAX_SHARED_ROOM_BUSES,
            active_streams: self
                .voices
                .values()
                .filter(|voice| matches!(voice.source, VoiceSource::Stream { .. }))
                .count(),
            physical_streams: self
                .voices
                .values()
                .filter(|voice| {
                    matches!(voice.source, VoiceSource::Stream { .. }) && voice.is_physical()
                })
                .count(),
            virtual_streams: self
                .voices
                .values()
                .filter(|voice| {
                    matches!(voice.source, VoiceSource::Stream { .. }) && voice.is_virtual()
                })
                .count(),
            stream_promotions: self.stream_promotions,
            stream_demotions: self.stream_demotions,
            stream_buffered_frames: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.buffered_frames())
                .sum(),
            stream_buffer_capacity_frames: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.capacity_frames())
                .sum(),
            stream_underruns: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.underruns())
                .sum(),
            stream_range_requests: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.range_requests())
                .sum(),
            stream_compressed_bytes_fetched: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.compressed_bytes_fetched())
                .sum(),
            stream_seek_operations: self
                .voices
                .values()
                .filter_map(|voice| voice.stream_stats.as_ref())
                .map(|stats| stats.seek_operations())
                .sum(),
            cached_sound_graphs: self.cue_sound_graphs.len(),
            sound_graph_sequence_states: self.sound_graph_sequences.len(),
            cached_clips: self.clips.len(),
            cached_bytes: self.cached_bytes,
            listener: self.listener,
            listener_velocity: self.listener_velocity,
            bus_gains: self
                .bus_gains
                .iter()
                .map(|(bus, gain)| (bus.as_str().to_owned(), *gain))
                .collect(),
        }
    }

    fn shutdown(&mut self) {
        for voice in self.voices.values() {
            if let Some(control) = voice.control.as_ref() {
                control.stop();
            }
        }
        self.voices.clear();
        self.stream_metadata.clear();
        self.clips.clear();
        self.cues.clear();
        self.cue_layers.clear();
        self.cue_clips_by_name.clear();
        self.cue_sound_graphs.clear();
        self.sound_graph_sequences.clear();
        self.cue_meta.clear();
        self.cached_bytes = 0;
    }
}
