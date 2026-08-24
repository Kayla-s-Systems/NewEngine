use serde::{Deserialize, Serialize};

pub const AUDIO_ENVIRONMENT_ZONE_COMPONENT_TYPE: &str = "audio.environment_zone";
pub const AUDIO_PORTAL_COMPONENT_TYPE: &str = "audio.portal";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioReverbPreset {
    pub early_reflections_gain: f32,
    pub pre_delay_ms: f32,
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
            pre_delay_ms: 0.0,
            decay_seconds: 0.1,
            damping: 1.0,
            diffusion: 0.0,
        }
    }

    pub const fn room() -> Self {
        Self {
            early_reflections_gain: 0.22,
            pre_delay_ms: 11.0,
            decay_seconds: 0.85,
            damping: 0.58,
            diffusion: 0.68,
        }
    }

    pub const fn corridor() -> Self {
        Self {
            early_reflections_gain: 0.28,
            pre_delay_ms: 18.0,
            decay_seconds: 1.45,
            damping: 0.46,
            diffusion: 0.74,
        }
    }

    pub const fn concrete_hall() -> Self {
        Self {
            early_reflections_gain: 0.34,
            pre_delay_ms: 24.0,
            decay_seconds: 2.8,
            damping: 0.32,
            diffusion: 0.82,
        }
    }

    pub const fn metal_hangar() -> Self {
        Self {
            early_reflections_gain: 0.40,
            pre_delay_ms: 31.0,
            decay_seconds: 4.2,
            damping: 0.18,
            diffusion: 0.90,
        }
    }

    pub const fn outdoor() -> Self {
        Self {
            early_reflections_gain: 0.05,
            pre_delay_ms: 7.0,
            decay_seconds: 0.28,
            damping: 0.82,
            diffusion: 0.22,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            early_reflections_gain: finite_clamped(self.early_reflections_gain, 0.0, 0.0, 2.0),
            pre_delay_ms: finite_clamped(self.pre_delay_ms, 0.0, 0.0, 250.0),
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
            pre_delay_ms: lerp(a.pre_delay_ms, b.pre_delay_ms, t),
            decay_seconds: lerp(a.decay_seconds, b.decay_seconds, t),
            damping: lerp(a.damping, b.damping, t),
            diffusion: lerp(a.diffusion, b.diffusion, t),
        }
        .sanitized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioReverbSend {
    pub gain: f32,
    pub preset: AudioReverbPreset,
}

impl Default for AudioReverbSend {
    fn default() -> Self {
        Self {
            gain: 0.0,
            preset: AudioReverbPreset::dry(),
        }
    }
}

impl AudioReverbSend {
    pub fn sanitized(self) -> Self {
        Self {
            gain: finite_clamped(self.gain, 0.0, 0.0, 2.0),
            preset: self.preset.sanitized(),
        }
    }

    fn lerped(self, target: Self, t: f32) -> Self {
        let a = self.sanitized();
        let b = target.sanitized();
        Self {
            gain: lerp(a.gain, b.gain, t),
            preset: a.preset.lerped(b.preset, t),
        }
        .sanitized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEnvironmentState {
    pub source_send: AudioReverbSend,
    pub listener_send: AudioReverbSend,
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
                gain: 0.0,
                preset: AudioReverbPreset::dry(),
            },
            listener_send: AudioReverbSend {
                gain: 0.0,
                preset: AudioReverbPreset::dry(),
            },
            portal_gain: 1.0,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            source_send: self.source_send.sanitized(),
            listener_send: self.listener_send.sanitized(),
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
        self.send_gain = finite_clamped(self.send_gain, 1.0, 0.0, 2.0);
        self
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
        assert!((portal.route_gain() - 0.3).abs() < 1.0e-6);
        portal.enabled = false;
        assert_eq!(portal.route_gain(), 0.0);
    }

    #[test]
    fn environment_state_smooths_room_transitions() {
        let target = AudioEnvironmentState {
            source_send: AudioReverbSend {
                gain: 0.6,
                preset: AudioReverbPreset::metal_hangar(),
            },
            listener_send: AudioReverbSend {
                gain: 0.4,
                preset: AudioReverbPreset::room(),
            },
            portal_gain: 0.5,
        };
        let moved = AudioEnvironmentState::clear().smoothed_towards(target, 0.016, 0.2);
        assert!(moved.source_send.gain > 0.0 && moved.source_send.gain < 0.6);
        assert!(moved.portal_gain < 1.0 && moved.portal_gain > 0.5);
    }
}
