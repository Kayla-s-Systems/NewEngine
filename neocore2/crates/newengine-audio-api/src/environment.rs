use serde::{Deserialize, Serialize};

pub const AUDIO_ENVIRONMENT_ZONE_COMPONENT_TYPE: &str = "audio.environment_zone";
pub const AUDIO_PORTAL_COMPONENT_TYPE: &str = "audio.portal";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioReverbPreset {
    pub early_reflections_gain: f32,
    /// High-frequency energy retained by the first-order reflection field after boundary
    /// absorption. Late-reverb damping remains an independent room-tail parameter.
    pub early_reflections_high_frequency_gain: f32,
    pub pre_delay_ms: f32,
    /// Spread between the first and later first-order reflection arrivals. World geometry may
    /// override this authored baseline without changing the late-reverb decay contract.
    pub early_reflections_spread_ms: f32,
    pub decay_seconds: f32,
    pub damping: f32,
    pub diffusion: f32,
}

impl Default for AudioReverbPreset {
    fn default() -> Self {
        Self::dry()
    }
}

impl AudioReverbPreset {
    pub const fn dry() -> Self {
        Self {
            early_reflections_gain: 0.0,
            early_reflections_high_frequency_gain: 1.0,
            pre_delay_ms: 0.0,
            early_reflections_spread_ms: 0.0,
            decay_seconds: 0.1,
            damping: 1.0,
            diffusion: 0.0,
        }
    }

    pub const fn room() -> Self {
        Self {
            early_reflections_gain: 0.22,
            early_reflections_high_frequency_gain: 1.0,
            pre_delay_ms: 11.0,
            early_reflections_spread_ms: 8.0,
            decay_seconds: 0.85,
            damping: 0.58,
            diffusion: 0.68,
        }
    }

    pub const fn corridor() -> Self {
        Self {
            early_reflections_gain: 0.28,
            early_reflections_high_frequency_gain: 1.0,
            pre_delay_ms: 18.0,
            early_reflections_spread_ms: 16.0,
            decay_seconds: 1.45,
            damping: 0.46,
            diffusion: 0.74,
        }
    }

    pub const fn concrete_hall() -> Self {
        Self {
            early_reflections_gain: 0.34,
            early_reflections_high_frequency_gain: 1.0,
            pre_delay_ms: 24.0,
            early_reflections_spread_ms: 26.0,
            decay_seconds: 2.8,
            damping: 0.32,
            diffusion: 0.82,
        }
    }

    pub const fn metal_hangar() -> Self {
        Self {
            early_reflections_gain: 0.40,
            early_reflections_high_frequency_gain: 1.0,
            pre_delay_ms: 31.0,
            early_reflections_spread_ms: 38.0,
            decay_seconds: 4.2,
            damping: 0.18,
            diffusion: 0.90,
        }
    }

    pub const fn outdoor() -> Self {
        Self {
            early_reflections_gain: 0.05,
            early_reflections_high_frequency_gain: 1.0,
            pre_delay_ms: 7.0,
            early_reflections_spread_ms: 4.0,
            decay_seconds: 0.28,
            damping: 0.82,
            diffusion: 0.22,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            early_reflections_gain: finite_clamped(self.early_reflections_gain, 0.0, 0.0, 2.0),
            early_reflections_high_frequency_gain: finite_clamped(
                self.early_reflections_high_frequency_gain,
                1.0,
                0.0,
                1.0,
            ),
            pre_delay_ms: finite_clamped(self.pre_delay_ms, 0.0, 0.0, 250.0),
            early_reflections_spread_ms: finite_clamped(
                self.early_reflections_spread_ms,
                0.0,
                0.0,
                250.0,
            ),
            decay_seconds: finite_clamped(self.decay_seconds, 0.1, 0.05, 20.0),
            damping: finite_clamped(self.damping, 1.0, 0.0, 1.0),
            diffusion: finite_clamped(self.diffusion, 0.0, 0.0, 1.0),
        }
    }

    fn lerped(self, target: Self, t: f32) -> Self {
        let a = self.sanitized();
        let b = target.sanitized();
        Self {
            early_reflections_gain: lerp(a.early_reflections_gain, b.early_reflections_gain, t),
            early_reflections_high_frequency_gain: lerp(
                a.early_reflections_high_frequency_gain,
                b.early_reflections_high_frequency_gain,
                t,
            ),
            pre_delay_ms: lerp(a.pre_delay_ms, b.pre_delay_ms, t),
            early_reflections_spread_ms: lerp(
                a.early_reflections_spread_ms,
                b.early_reflections_spread_ms,
                t,
            ),
            decay_seconds: lerp(a.decay_seconds, b.decay_seconds, t),
            damping: lerp(a.damping, b.damping, t),
            diffusion: lerp(a.diffusion, b.diffusion, t),
        }
        .sanitized()
    }
}

pub const AUDIO_MAX_EARLY_REFLECTION_TAPS: usize = 8;

/// One authoritative discrete early reflection arrival. Higher-order path construction lives in
/// world audio; providers receive only bounded delay/energy/spectrum/direction semantics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEarlyReflectionTap {
    pub delay_ms: f32,
    pub gain: f32,
    pub high_frequency_gain: f32,
    pub direction: [f32; 3],
    pub order: u8,
}

impl Default for AudioEarlyReflectionTap {
    fn default() -> Self {
        Self::silent()
    }
}

impl AudioEarlyReflectionTap {
    pub const fn silent() -> Self {
        Self {
            delay_ms: 0.0,
            gain: 0.0,
            high_frequency_gain: 1.0,
            direction: [0.0; 3],
            order: 1,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            delay_ms: finite_clamped(self.delay_ms, 0.0, 0.0, 500.0),
            gain: finite_clamped(self.gain, 0.0, 0.0, 2.0),
            high_frequency_gain: finite_clamped(self.high_frequency_gain, 1.0, 0.0, 1.0),
            direction: sanitize_direction(self.direction),
            order: self.order.clamp(1, 2),
        }
    }

    fn lerped(self, target: Self, t: f32) -> Self {
        let a = self.sanitized();
        let b = target.sanitized();
        Self {
            delay_ms: lerp(a.delay_ms, b.delay_ms, t),
            gain: lerp(a.gain, b.gain, t),
            high_frequency_gain: lerp(a.high_frequency_gain, b.high_frequency_gain, t),
            direction: sanitize_direction([
                lerp(a.direction[0], b.direction[0], t),
                lerp(a.direction[1], b.direction[1], t),
                lerp(a.direction[2], b.direction[2], t),
            ]),
            order: if t < 0.5 { a.order } else { b.order },
        }
        .sanitized()
    }
}

/// Fixed-capacity early reflection field. Fixed storage keeps the real-time provider contract
/// allocation-free and preserves Copy semantics for per-voice environment snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEarlyReflectionField {
    pub count: u8,
    pub taps: [AudioEarlyReflectionTap; AUDIO_MAX_EARLY_REFLECTION_TAPS],
}

impl Default for AudioEarlyReflectionField {
    fn default() -> Self {
        Self::empty()
    }
}

impl AudioEarlyReflectionField {
    pub const fn empty() -> Self {
        Self {
            count: 0,
            taps: [AudioEarlyReflectionTap::silent(); AUDIO_MAX_EARLY_REFLECTION_TAPS],
        }
    }

    pub fn sanitized(self) -> Self {
        let count = usize::from(self.count).min(AUDIO_MAX_EARLY_REFLECTION_TAPS);
        let mut taps = self.taps.map(AudioEarlyReflectionTap::sanitized);
        taps[..count].sort_by(|a, b| {
            a.delay_ms
                .total_cmp(&b.delay_ms)
                .then_with(|| a.order.cmp(&b.order))
                .then_with(|| b.gain.total_cmp(&a.gain))
        });
        for tap in &mut taps[count..] {
            *tap = AudioEarlyReflectionTap::default();
        }
        Self {
            count: count as u8,
            taps,
        }
    }

    pub fn active(&self) -> &[AudioEarlyReflectionTap] {
        &self.taps[..usize::from(self.count).min(AUDIO_MAX_EARLY_REFLECTION_TAPS)]
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn lerped(self, target: Self, t: f32) -> Self {
        let a = self.sanitized();
        let b = target.sanitized();
        let a_count = usize::from(a.count);
        let b_count = usize::from(b.count);
        let count = a_count.max(b_count);
        let mut taps = [AudioEarlyReflectionTap::default(); AUDIO_MAX_EARLY_REFLECTION_TAPS];
        for (index, tap) in taps.iter_mut().enumerate().take(count) {
            *tap = match (index < a_count, index < b_count) {
                (true, true) => a.taps[index].lerped(b.taps[index], t),
                (true, false) => {
                    let mut fading = a.taps[index];
                    fading.gain *= 1.0 - t;
                    fading.sanitized()
                }
                (false, true) => {
                    let mut fading = b.taps[index];
                    fading.gain *= t;
                    fading.sanitized()
                }
                (false, false) => AudioEarlyReflectionTap::default(),
            };
        }
        Self {
            count: count as u8,
            taps,
        }
        .sanitized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioReverbSend {
    /// Stable provider-neutral acoustic room identity for shared late-field processing.
    /// Zero disables shared late-bus routing and preserves legacy per-send behavior.
    pub room_bus_id: u64,
    pub gain: f32,
    pub preset: AudioReverbPreset,
    /// Authoritative bounded early path field. Empty means use legacy preset-derived early taps.
    pub early_reflections: AudioEarlyReflectionField,
    /// World-space unit vector from listener toward the apparent first-order arrival point.
    /// Zero means diffuse/unknown direction and deliberately does not invent a pan.
    pub early_reflection_direction: [f32; 3],
}

impl Default for AudioReverbSend {
    fn default() -> Self {
        Self {
            room_bus_id: 0,
            gain: 0.0,
            preset: AudioReverbPreset::dry(),
            early_reflections: AudioEarlyReflectionField::default(),
            early_reflection_direction: [0.0; 3],
        }
    }
}

impl AudioReverbSend {
    pub fn sanitized(self) -> Self {
        Self {
            room_bus_id: self.room_bus_id,
            gain: finite_clamped(self.gain, 0.0, 0.0, 2.0),
            preset: self.preset.sanitized(),
            early_reflections: self.early_reflections.sanitized(),
            early_reflection_direction: sanitize_direction(self.early_reflection_direction),
        }
    }

    fn lerped(self, target: Self, t: f32) -> Self {
        let a = self.sanitized();
        let b = target.sanitized();
        Self {
            // Room identity is not interpolatable. Route the smoothed send into the target room
            // immediately; the old shared bus is then free to decay naturally.
            room_bus_id: b.room_bus_id,
            gain: lerp(a.gain, b.gain, t),
            preset: a.preset.lerped(b.preset, t),
            early_reflections: a.early_reflections.lerped(b.early_reflections, t),
            early_reflection_direction: sanitize_direction([
                lerp(
                    a.early_reflection_direction[0],
                    b.early_reflection_direction[0],
                    t,
                ),
                lerp(
                    a.early_reflection_direction[1],
                    b.early_reflection_direction[1],
                    t,
                ),
                lerp(
                    a.early_reflection_direction[2],
                    b.early_reflection_direction[2],
                    t,
                ),
            ]),
        }
        .sanitized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioDirectPathResponse {
    /// Broadband energy retained by the resolved geometric portal/diffraction path.
    pub gain: f32,
    /// High-frequency shelf retained after edge diffraction.
    pub high_frequency_gain: f32,
    /// Low-pass cutoff introduced by the alternate path.
    pub low_pass_hz: f32,
    /// Extra travel time relative to the straight source/listener segment.
    pub extra_delay_ms: f32,
}

impl Default for AudioDirectPathResponse {
    fn default() -> Self {
        Self::clear()
    }
}

impl AudioDirectPathResponse {
    pub const fn clear() -> Self {
        Self {
            gain: 1.0,
            high_frequency_gain: 1.0,
            low_pass_hz: 20_000.0,
            extra_delay_ms: 0.0,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            gain: finite_clamped(self.gain, 1.0, 0.0, 1.0),
            high_frequency_gain: finite_clamped(self.high_frequency_gain, 1.0, 0.0, 1.0),
            low_pass_hz: finite_clamped(self.low_pass_hz, 20_000.0, 80.0, 20_000.0),
            extra_delay_ms: finite_clamped(self.extra_delay_ms, 0.0, 0.0, 500.0),
        }
    }

    fn lerped(self, target: Self, t: f32) -> Self {
        let a = self.sanitized();
        let b = target.sanitized();
        Self {
            gain: lerp(a.gain, b.gain, t),
            high_frequency_gain: lerp(a.high_frequency_gain, b.high_frequency_gain, t),
            low_pass_hz: lerp(a.low_pass_hz, b.low_pass_hz, t),
            extra_delay_ms: lerp(a.extra_delay_ms, b.extra_delay_ms, t),
        }
        .sanitized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEnvironmentState {
    pub source_send: AudioReverbSend,
    pub listener_send: AudioReverbSend,
    /// Generic alternate direct-path response. Clear/same-room paths are unity.
    pub direct_path: AudioDirectPathResponse,
    /// Strongest portal graph gain retained for diagnostics and indirect-send routing.
    pub portal_gain: f32,
}

impl Default for AudioEnvironmentState {
    fn default() -> Self {
        Self::clear()
    }
}

impl AudioEnvironmentState {
    pub const fn clear() -> Self {
        Self {
            source_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 0.0,
                preset: AudioReverbPreset::dry(),
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            listener_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 0.0,
                preset: AudioReverbPreset::dry(),
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            direct_path: AudioDirectPathResponse::clear(),
            portal_gain: 1.0,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            source_send: self.source_send.sanitized(),
            listener_send: self.listener_send.sanitized(),
            direct_path: self.direct_path.sanitized(),
            portal_gain: finite_clamped(self.portal_gain, 1.0, 0.0, 1.0),
        }
    }

    pub fn smoothed_towards(self, target: Self, dt: f32, transition_seconds: f32) -> Self {
        let current = self.sanitized();
        let target = target.sanitized();
        let dt = finite_clamped(dt, 1.0 / 60.0, 0.0, 0.25);
        let time = finite_clamped(transition_seconds, 0.18, 0.01, 10.0);
        let alpha = if dt <= 0.0 {
            0.0
        } else {
            1.0 - (-dt / time).exp()
        };
        Self {
            source_send: current.source_send.lerped(target.source_send, alpha),
            listener_send: current.listener_send.lerped(target.listener_send, alpha),
            direct_path: current.direct_path.lerped(target.direct_path, alpha),
            portal_gain: lerp(current.portal_gain, target.portal_gain, alpha),
        }
        .sanitized()
    }

    pub fn is_wet(self) -> bool {
        let state = self.sanitized();
        state.source_send.gain > 1.0e-4 || state.listener_send.gain > 1.0e-4
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioEnvironmentKind {
    #[default]
    Indoor,
    Outdoor,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEnvironmentZone {
    pub zone_id: String,
    pub enabled: bool,
    pub kind: AudioEnvironmentKind,
    pub half_extents: [f32; 3],
    pub priority: i32,
    pub blend_distance: f32,
    pub send_gain: f32,
    pub transition_seconds: f32,
    pub reverb: AudioReverbPreset,
}

impl Default for AudioEnvironmentZone {
    fn default() -> Self {
        Self {
            zone_id: "environment.default".to_owned(),
            enabled: true,
            kind: AudioEnvironmentKind::Indoor,
            half_extents: [5.0, 3.0, 5.0],
            priority: 0,
            blend_distance: 0.75,
            send_gain: 0.35,
            transition_seconds: 0.18,
            reverb: AudioReverbPreset::room(),
        }
    }
}

impl AudioEnvironmentZone {
    pub fn new(zone_id: impl Into<String>, half_extents: [f32; 3]) -> Self {
        Self {
            zone_id: zone_id.into(),
            half_extents,
            ..Self::default()
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.zone_id = self.zone_id.trim().to_owned();
        if self.zone_id.is_empty() {
            self.zone_id = "environment.default".to_owned();
        }
        self.half_extents = self
            .half_extents
            .map(|value| finite_clamped(value.abs(), 1.0, 0.05, 100_000.0));
        self.blend_distance = finite_clamped(self.blend_distance, 0.75, 0.0, 10_000.0);
        self.send_gain = finite_clamped(self.send_gain, 0.35, 0.0, 2.0);
        self.transition_seconds = finite_clamped(self.transition_seconds, 0.18, 0.01, 10.0);
        self.reverb = self.reverb.sanitized();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioPortal {
    pub portal_id: String,
    pub enabled: bool,
    pub zone_a: String,
    pub zone_b: String,
    pub openness: f32,
    pub transmission_gain: f32,
    /// Authored portal aperture half-size in local width/height meters. The entity Transform owns
    /// world position/orientation; these dimensions only describe the opening.
    pub half_extents: [f32; 2],
    pub send_gain: f32,
}

impl Default for AudioPortal {
    fn default() -> Self {
        Self {
            portal_id: "portal.default".to_owned(),
            enabled: true,
            zone_a: String::new(),
            zone_b: String::new(),
            openness: 1.0,
            transmission_gain: 1.0,
            half_extents: [0.6, 1.0],
            send_gain: 1.0,
        }
    }
}

impl AudioPortal {
    pub fn new(
        portal_id: impl Into<String>,
        zone_a: impl Into<String>,
        zone_b: impl Into<String>,
    ) -> Self {
        Self {
            portal_id: portal_id.into(),
            zone_a: zone_a.into(),
            zone_b: zone_b.into(),
            ..Self::default()
        }
        .sanitized()
    }

    pub fn sanitized(mut self) -> Self {
        self.portal_id = self.portal_id.trim().to_owned();
        if self.portal_id.is_empty() {
            self.portal_id = "portal.default".to_owned();
        }
        self.zone_a = self.zone_a.trim().to_owned();
        self.zone_b = self.zone_b.trim().to_owned();
        self.openness = finite_clamped(self.openness, 1.0, 0.0, 1.0);
        self.transmission_gain = finite_clamped(self.transmission_gain, 1.0, 0.0, 1.0);
        self.half_extents = self
            .half_extents
            .map(|value| finite_clamped(value.abs(), 0.5, 0.01, 100.0));
        self.send_gain = finite_clamped(self.send_gain, 1.0, 0.0, 2.0);
        self
    }

    pub fn direct_route_gain(&self) -> f32 {
        let portal = self.clone().sanitized();
        if !portal.enabled || portal.zone_a.is_empty() || portal.zone_b.is_empty() {
            0.0
        } else {
            (portal.openness * portal.transmission_gain).clamp(0.0, 1.0)
        }
    }

    pub fn route_gain(&self) -> f32 {
        let portal = self.clone().sanitized();
        if !portal.enabled || portal.zone_a.is_empty() || portal.zone_b.is_empty() {
            0.0
        } else {
            (portal.openness * portal.transmission_gain * portal.send_gain).clamp(0.0, 1.0)
        }
    }
}

fn sanitize_direction(value: [f32; 3]) -> [f32; 3] {
    if !value.iter().all(|component| component.is_finite()) {
        return [0.0; 3];
    }
    let length_sq = value[0] * value[0] + value[1] * value[1] + value[2] * value[2];
    if length_sq <= 1.0e-10 {
        [0.0; 3]
    } else {
        let inv = length_sq.sqrt().recip();
        [value[0] * inv, value[1] * inv, value[2] * inv]
    }
}

fn finite_clamped(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_presets_are_distinct_and_bounded() {
        let room = AudioReverbPreset::room().sanitized();
        let hall = AudioReverbPreset::concrete_hall().sanitized();
        let hangar = AudioReverbPreset::metal_hangar().sanitized();
        assert!(room.decay_seconds < hall.decay_seconds);
        assert!(hall.decay_seconds < hangar.decay_seconds);
        assert!(hangar.damping < room.damping);
    }

    #[test]
    fn portal_gain_tracks_openness_and_transmission() {
        let mut portal = AudioPortal::new("door", "room.a", "room.b");
        portal.openness = 0.5;
        portal.transmission_gain = 0.8;
        portal.send_gain = 0.75;
        assert!((portal.direct_route_gain() - 0.4).abs() < 1.0e-6);
        assert!((portal.route_gain() - 0.3).abs() < 1.0e-6);
        portal.send_gain = 0.0;
        assert!((portal.direct_route_gain() - 0.4).abs() < 1.0e-6);
        assert_eq!(portal.route_gain(), 0.0);
        portal.enabled = false;
        assert_eq!(portal.direct_route_gain(), 0.0);
        assert_eq!(portal.route_gain(), 0.0);
    }

    #[test]
    fn environment_state_smooths_room_transitions() {
        let target = AudioEnvironmentState {
            source_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 0.6,
                preset: AudioReverbPreset::metal_hangar(),
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            listener_send: AudioReverbSend {
                room_bus_id: 0,
                gain: 0.4,
                preset: AudioReverbPreset::room(),
                early_reflections: AudioEarlyReflectionField::empty(),
                early_reflection_direction: [0.0; 3],
            },
            direct_path: AudioDirectPathResponse {
                gain: 0.5,
                ..AudioDirectPathResponse::clear()
            },
            portal_gain: 0.5,
        };
        let moved = AudioEnvironmentState::clear().smoothed_towards(target, 0.016, 0.2);
        assert!(moved.source_send.gain > 0.0 && moved.source_send.gain < 0.6);
        assert!(moved.direct_path.gain < 1.0 && moved.direct_path.gain > 0.5);
        assert!(moved.portal_gain < 1.0 && moved.portal_gain > 0.5);
    }

    #[test]
    fn explicit_early_reflection_field_is_bounded_sorted_and_sanitized() {
        let mut field = AudioEarlyReflectionField {
            count: 20,
            ..AudioEarlyReflectionField::default()
        };
        field.taps[0] = AudioEarlyReflectionTap {
            delay_ms: 30.0,
            gain: 0.4,
            high_frequency_gain: 0.8,
            direction: [3.0, 0.0, 0.0],
            order: 1,
        };
        field.taps[1] = AudioEarlyReflectionTap {
            delay_ms: 10.0,
            gain: 0.2,
            high_frequency_gain: 2.0,
            direction: [0.0, 0.0, 0.0],
            order: 9,
        };
        let field = field.sanitized();
        assert_eq!(usize::from(field.count), AUDIO_MAX_EARLY_REFLECTION_TAPS);
        assert_eq!(field.taps[0].delay_ms, 0.0);
        assert!(field
            .active()
            .windows(2)
            .all(|pair| pair[0].delay_ms <= pair[1].delay_ms));
        let delayed = field
            .active()
            .iter()
            .find(|tap| tap.delay_ms == 10.0)
            .unwrap();
        assert_eq!(delayed.high_frequency_gain, 1.0);
        assert_eq!(delayed.order, 2);
        let directional = field
            .active()
            .iter()
            .find(|tap| tap.delay_ms == 30.0)
            .unwrap();
        assert!((directional.direction[0] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn early_reflection_field_fades_topology_changes_without_allocating() {
        let mut target = AudioEarlyReflectionField {
            count: 1,
            ..AudioEarlyReflectionField::default()
        };
        target.taps[0] = AudioEarlyReflectionTap {
            delay_ms: 20.0,
            gain: 0.8,
            high_frequency_gain: 0.5,
            direction: [1.0, 0.0, 0.0],
            order: 2,
        };
        let halfway = AudioEarlyReflectionField::default().lerped(target, 0.5);
        assert_eq!(halfway.count, 1);
        assert!((halfway.taps[0].gain - 0.4).abs() < 1.0e-6);
        assert_eq!(halfway.taps[0].delay_ms, 20.0);
        assert_eq!(halfway.taps[0].order, 2);
    }
}
