impl AudioRuntimeState {
    fn preload(&mut self, request: AudioPreloadRequest) -> Result<AudioPreloadAck, String> {
        let uri = normalize_vfs_path(&request.clip.uri)?;
        if let Some(existing) = self.clips.get(&uri) {
            return Ok(AudioPreloadAck {
                accepted: true,
                cached: true,
                bytes: existing.len(),
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                diagnostics: Vec::new(),
            });
        }

        let bytes = if let Some(locator) = self.embedded_yscd_clips.get(&uri).cloned() {
            self.read_embedded_yscd_clip(&locator)?
        } else {
            self.assets
                .raw_bytes_v1(&uri)
                .map_err(|error| format!("audio VFS read failed logical_path='{uri}': {error}"))?
        };
        self.cache_clip_bytes(uri, bytes)
    }

    fn cache_clip_bytes(&mut self, uri: String, bytes: Vec<u8>) -> Result<AudioPreloadAck, String> {
        if let Some(existing) = self.clips.get(&uri) {
            return Ok(AudioPreloadAck {
                accepted: true,
                cached: true,
                bytes: existing.len(),
                provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
                diagnostics: Vec::new(),
            });
        }
        if bytes.is_empty() {
            return Err(format!("audio clip is empty: '{uri}'"));
        }
        if bytes.len() > self.cache_limit_bytes {
            return Err(format!(
                "audio clip '{uri}' is {} bytes and exceeds cache limit {} bytes",
                bytes.len(),
                self.cache_limit_bytes
            ));
        }
        self.make_cache_room(bytes.len());
        let bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let len = bytes.len();
        self.clips.insert(
            uri,
            CachedClip {
                bytes,
                source_duration: OnceLock::new(),
            },
        );
        self.cached_bytes = self.cached_bytes.saturating_add(len);
        Ok(AudioPreloadAck {
            accepted: true,
            cached: false,
            bytes: len,
            provider: NATIVE_AUDIO_PROVIDER_ROUTE.to_owned(),
            diagnostics: Vec::new(),
        })
    }

    fn read_embedded_yscd_clip(
        &self,
        locator: &EmbeddedYscdClipLocator,
    ) -> Result<Vec<u8>, String> {
        let source = self
            .assets
            .raw_bytes_v1(&locator.dictionary_path)
            .map_err(|error| {
                format!(
                    "YSCD VFS read failed dictionary='{}' cue='{}': {error}",
                    locator.dictionary_path, locator.cue_name
                )
            })?;
        let dictionary =
            newengine_asset_format_nef8::decode_yscd_nef8(&source, &locator.dictionary_path)?;
        let cue = dictionary.cue(&locator.cue_name).ok_or_else(|| {
            format!(
                "YSCD cue '{}' not found in '{}'",
                locator.cue_name, locator.dictionary_path
            )
        })?;
        cue.clips
            .get(locator.clip_index)
            .map(|clip| clip.bytes.clone())
            .ok_or_else(|| {
                format!(
                    "YSCD cue '{}' clip index {} out of range in '{}'",
                    locator.cue_name, locator.clip_index, locator.dictionary_path
                )
            })
    }

    fn make_cache_room(&mut self, incoming: usize) {
        if self.cached_bytes.saturating_add(incoming) <= self.cache_limit_bytes {
            return;
        }
        // V1 uses a deterministic all-or-nothing eviction. LRU/residency belongs
        // in the shared asset/VFS layer rather than leaking into the provider API.
        self.clips.clear();
        self.cues.clear();
        self.cue_layers.clear();
        self.cue_clips_by_name.clear();
        self.cue_sound_graphs.clear();
        self.cue_meta.clear();
        self.cached_bytes = 0;
    }

    fn clip_bytes(&mut self, uri: &str) -> Result<Arc<[u8]>, String> {
        let normalized = normalize_vfs_path(uri)?;
        if !self.clips.contains_key(&normalized) {
            self.preload(AudioPreloadRequest {
                clip: newengine_audio_api::AudioClipRef::new(normalized.clone()),
            })?;
        }
        self.clips
            .get(&normalized)
            .map(|clip| Arc::clone(&clip.bytes))
            .ok_or_else(|| format!("audio clip cache admission failed: '{normalized}'"))
    }

    fn clip_source_duration(&mut self, uri: &str) -> Result<Option<Duration>, String> {
        let normalized = normalize_vfs_path(uri)?;
        if !self.clips.contains_key(&normalized) {
            let _ = self.clip_bytes(&normalized)?;
        }
        if let Some(duration) = self
            .clips
            .get(&normalized)
            .and_then(|clip| clip.source_duration.get().copied())
        {
            return Ok(duration);
        }
        let bytes = self
            .clips
            .get(&normalized)
            .map(|clip| Arc::clone(&clip.bytes))
            .ok_or_else(|| format!("audio clip cache admission failed: '{normalized}'"))?;
        let decoder = Decoder::try_from(Cursor::new(bytes))
            .map_err(|error| format!("audio decode failed '{normalized}': {error}"))?;
        let duration = decoder.total_duration();
        if let Some(clip) = self.clips.get(&normalized) {
            let _ = clip.source_duration.set(duration);
        }
        Ok(duration)
    }
}
