fn normalize_vfs_path(uri: &str) -> Result<String, String> {
    let reference = newengine_assets_api::parse_asset_reference(uri)
        .map_err(|error| format!("audio references must use VFS logical paths: {error}"))?;
    if reference.entry.is_some() {
        return Err(format!(
            "audio clip/cue reference '{}' must address a file, not an @entry",
            reference.canonical
        ));
    }
    Ok(reference.logical_path)
}

fn select_weighted_clip(cue: &SoundCue, unit: f32) -> Option<&SoundCueClip> {
    select_weighted_clips(&cue.clips, unit)
}

fn select_weighted_clips(clips: &[SoundCueClip], unit: f32) -> Option<&SoundCueClip> {
    let total = clips.iter().map(|clip| clip.weight.max(0.0)).sum::<f32>();
    if !(total.is_finite() && total > 0.0) {
        return None;
    }
    let mut cursor = unit.clamp(0.0, 0.999_999_94) * total;
    for clip in clips {
        cursor -= clip.weight.max(0.0);
        if cursor <= 0.0 {
            return Some(clip);
        }
    }
    clips.last()
}

fn embedded_yscd_clip_key(cue_reference: &str, clip_index: usize, codec: &str) -> String {
    let hash = stable_text_hash(cue_reference);
    let codec = codec.trim().trim_start_matches('.').to_ascii_lowercase();
    if codec.is_empty() {
        format!("__yscd/{hash:016x}/{clip_index:04}")
    } else {
        format!("__yscd/{hash:016x}/{clip_index:04}.{codec}")
    }
}

fn audio_bus_from_yscd(value: &str) -> Result<AudioBus, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "master" => Ok(AudioBus::Master),
        "music" => Ok(AudioBus::Music),
        "sfx" => Ok(AudioBus::Sfx),
        "ui" => Ok(AudioBus::Ui),
        "dialogue" => Ok(AudioBus::Dialogue),
        "ambience" => Ok(AudioBus::Ambience),
        other => Err(format!("YSCD cue has unsupported audio bus '{other}'")),
    }
}

fn sound_cue_spatial_policy_from_yscd(value: &str) -> Result<SoundCueSpatialPolicy, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "inherit" => Ok(SoundCueSpatialPolicy::Inherit),
        "non_spatial" | "nonspatial" | "2d" => Ok(SoundCueSpatialPolicy::NonSpatial),
        "spatial" | "3d" => Ok(SoundCueSpatialPolicy::Spatial),
        other => Err(format!("YSCD cue has unsupported spatial_policy '{other}'")),
    }
}

fn audio_attenuation_from_yscd(
    authored: &newengine_asset_format_nef8::YscdAttenuation,
) -> Result<AudioAttenuationSettings, String> {
    let curve = match authored.curve.trim().to_ascii_lowercase().as_str() {
        "linear" => newengine_audio_api::AudioAttenuationCurve::Linear,
        "smoothstep" => newengine_audio_api::AudioAttenuationCurve::Smoothstep,
        "inverse" => newengine_audio_api::AudioAttenuationCurve::Inverse,
        "exponential" => newengine_audio_api::AudioAttenuationCurve::Exponential,
        "custom" => newengine_audio_api::AudioAttenuationCurve::Custom,
        other => {
            return Err(format!(
                "YSCD cue has unsupported attenuation curve '{other}'"
            ));
        }
    };
    Ok(AudioAttenuationSettings {
        min_distance: authored.min_distance,
        max_distance: authored.max_distance,
        curve,
        rolloff: authored.rolloff,
        curve_points: authored.curve_points.clone(),
    }
    .sanitized())
}

#[inline]
fn sample_range(range: [f32; 2], unit: f32) -> f32 {
    range[0] + (range[1] - range[0]) * unit.clamp(0.0, 1.0)
}

#[inline]
fn unit_f32(value: u64) -> f32 {
    ((value >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

#[inline]
fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn stable_text_hash(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[inline]
fn sanitize_position(position: [f32; 3]) -> [f32; 3] {
    position.map(|component| {
        if component.is_finite() {
            component
        } else {
            0.0
        }
    })
}

#[inline]
fn distance3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[inline]
fn should_seek_materialized_voice(position: Duration) -> bool {
    position >= Duration::from_millis(MIN_MATERIALIZE_SEEK_MS)
}

fn max_physical_voices_from_env() -> usize {
    newengine_plugin_host::current_host_context()
        .environment_var("NEWENGINE_AUDIO_MAX_PHYSICAL_VOICES")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|voices| voices.clamp(1, MAX_CONFIGURED_PHYSICAL_VOICES))
        .unwrap_or(DEFAULT_MAX_PHYSICAL_VOICES)
}

fn feedback_tone(event_id: &str) -> (f32, u64) {
    match event_id {
        "ui.open" => (660.0, 55),
        "ui.close" => (440.0, 50),
        "ui.navigate" => (520.0, 30),
        "ui.confirm" => (780.0, 70),
        "ui.back" => (390.0, 55),
        "ui.rebind" => (880.0, 85),
        "ui.error" => (220.0, 120),
        _ => (500.0, 35),
    }
}

fn cache_limit_bytes_from_env() -> usize {
    newengine_plugin_host::current_host_context()
        .environment_var("NEWENGINE_AUDIO_CACHE_MB")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .map(|mb| mb.clamp(8, 2048).saturating_mul(1024 * 1024))
        .unwrap_or(DEFAULT_CLIP_CACHE_LIMIT_BYTES)
}

#[inline]
fn headless_runtime() -> bool {
    env_flag("NEWENGINE_HEADLESS")
}

#[inline]
fn audio_disabled_by_env() -> bool {
    env_flag("NEWENGINE_AUDIO_DISABLED")
}

fn env_flag(name: &str) -> bool {
    newengine_plugin_host::current_host_context()
        .environment_var(name)
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}
