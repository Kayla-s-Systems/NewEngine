#[derive(Clone, Debug)]
struct DecodedNativeClip {
    channels: ChannelCount,
    sample_rate: SampleRate,
    samples: Arc<[f32]>,
}

impl DecodedNativeClip {
    #[inline]
    fn source(self: &Arc<Self>) -> SharedPcmSource {
        SharedPcmSource {
            clip: Arc::clone(self),
            cursor: 0,
        }
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        let channels = usize::from(self.channels.get()).max(1);
        let frames = self.samples.len() / channels;
        let rate = self.sample_rate.get();
        (rate > 0).then(|| Duration::from_secs_f64(frames as f64 / rate as f64))
    }
}

#[derive(Clone, Debug)]
struct SharedPcmSource {
    clip: Arc<DecodedNativeClip>,
    cursor: usize,
}

impl Iterator for SharedPcmSource {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.clip.samples.get(self.cursor).copied()?;
        self.cursor += 1;
        Some(sample)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.clip.samples.len().saturating_sub(self.cursor);
        (remaining, Some(remaining))
    }
}

impl Source for SharedPcmSource {
    #[inline]
    fn current_span_len(&self) -> Option<usize> {
        Some(self.clip.samples.len().saturating_sub(self.cursor))
    }

    #[inline]
    fn channels(&self) -> ChannelCount {
        self.clip.channels
    }

    #[inline]
    fn sample_rate(&self) -> SampleRate {
        self.clip.sample_rate
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        self.clip.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        let channels = usize::from(self.clip.channels.get()).max(1);
        let frames = self.clip.samples.len() / channels;
        let target_frame = (position.as_secs_f64() * self.clip.sample_rate.get() as f64)
            .floor()
            .max(0.0) as usize;
        self.cursor = target_frame.min(frames).saturating_mul(channels);
        Ok(())
    }
}

fn decode_native_clip_pcm(
    assets: &AssetServiceClient,
    uri: &str,
    bytes: &[u8],
) -> Result<Option<Arc<DecodedNativeClip>>, String> {
    let (channels, sample_rate_hz, samples) = if bytes.get(0..4) == Some(b"XVAG") {
        let clip = newengine_audio_xvag::decode_xvag_ps_adpcm(bytes)
            .map_err(|error| format!("XVAG decode failed '{uri}': {error}"))?;
        (clip.channels, clip.sample_rate_hz, clip.samples)
    } else if bytes.get(0..4) == Some(b"NEF8") {
        let descriptor = assets
            .resolve_file_type_v1(uri)
            .map_err(|error| format!("native audio format resolve failed '{uri}': {error}"))?;
        if !descriptor.semantic_gateway.eq_ignore_ascii_case(ENGINE_AUDIO_SERVICE_ID)
            || !descriptor.asset_kind.eq_ignore_ascii_case("audio_clip")
        {
            return Err(format!(
                "NEF8 asset '{uri}' is not a native audio clip: kind='{}' gateway='{}'",
                descriptor.asset_kind, descriptor.semantic_gateway
            ));
        }
        let content_kind = descriptor.content_kind.ok_or_else(|| {
            format!(
                "native audio format module '{}' does not declare NEF8 content_kind",
                descriptor.module_id
            )
        })?;
        let schema_version = descriptor.content_schema_version.ok_or_else(|| {
            format!(
                "native audio format module '{}' does not declare content_schema_version",
                descriptor.module_id
            )
        })?;
        let clip = newengine_asset_format_nef8::decode_audio_clip_nef8(
            bytes,
            uri,
            content_kind,
            schema_version,
        )?;
        (clip.channels, clip.sample_rate_hz, clip.samples)
    } else {
        return Ok(None);
    };

    let channels = ChannelCount::new(channels)
        .ok_or_else(|| format!("native audio clip '{uri}' has zero channels"))?;
    let sample_rate = SampleRate::new(sample_rate_hz)
        .ok_or_else(|| format!("native audio clip '{uri}' has zero sample rate"))?;
    Ok(Some(Arc::new(DecodedNativeClip {
        channels,
        sample_rate,
        samples: Arc::from(samples.into_boxed_slice()),
    })))
}

fn decode_generic_clip_source(
    uri: &str,
    bytes: Arc<[u8]>,
) -> Result<Box<dyn Source + Send>, String> {
    let decoder = Decoder::try_from(Cursor::new(bytes))
        .map_err(|error| format!("audio decode failed '{uri}': {error}"))?;
    Ok(Box::new(decoder))
}

#[cfg(test)]
mod native_clip_cache_tests {
    use super::*;

    #[test]
    fn shared_pcm_sources_have_independent_cursors_and_seek() {
        let clip = Arc::new(DecodedNativeClip {
            channels: ChannelCount::new(1).unwrap(),
            sample_rate: SampleRate::new(4).unwrap(),
            samples: Arc::from(vec![0.0_f32, 0.25, 0.5, 0.75].into_boxed_slice()),
        });
        let mut a = clip.source();
        let mut b = clip.source();
        assert_eq!(a.next(), Some(0.0));
        assert_eq!(a.next(), Some(0.25));
        assert_eq!(b.next(), Some(0.0));
        b.try_seek(Duration::from_millis(500)).unwrap();
        assert_eq!(b.next(), Some(0.5));
        assert_eq!(a.next(), Some(0.5));
    }
}
