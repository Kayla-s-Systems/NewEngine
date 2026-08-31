use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{AudioParameterSet, AudioPlayStreamInstanceRequest, AudioTransportSchedulePoint};

pub const AUDIO_INTERACTIVE_MUSIC_SCHEMA: &str = "newengine.audio.interactive-music.v1";
pub const AUDIO_INTERACTIVE_MUSIC_VERSION: u32 = 1;
pub const AUDIO_INTERACTIVE_MUSIC_CAPABILITY_ID: &str = "audio.interactive-music";
pub const AUDIO_INTERACTIVE_MUSIC_MAX_STEMS: usize = 64;
pub const AUDIO_INTERACTIVE_MUSIC_MAX_STATES: usize = 128;
pub const AUDIO_INTERACTIVE_MUSIC_MAX_TRANSITIONS: usize = 512;
pub const AUDIO_INTERACTIVE_MUSIC_MAX_SELECTORS: usize = 256;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct AudioMusicSessionId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMusicStemSpec {
    pub id: String,
    pub request: AudioPlayStreamInstanceRequest,
}

impl Default for AudioMusicStemSpec {
    fn default() -> Self {
        Self {
            id: String::new(),
            request: AudioPlayStreamInstanceRequest::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMusicLayerSpec {
    pub stem: String,
    pub gain: f32,
}

impl Default for AudioMusicLayerSpec {
    fn default() -> Self {
        Self {
            stem: String::new(),
            gain: 1.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMusicStateSpec {
    pub id: String,
    pub layers: Vec<AudioMusicLayerSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMusicTransitionSpec {
    pub from: String,
    pub to: String,
    pub quantization: AudioTransportSchedulePoint,
    pub crossfade_samples: u64,
}

impl Default for AudioMusicTransitionSpec {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            quantization: AudioTransportSchedulePoint::NextBar,
            crossfade_samples: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case")]
pub enum AudioMusicSelectorCondition {
    SwitchEquals { name: String, value: String },
    ScalarRange { name: String, min: f32, max: f32 },
}

impl AudioMusicSelectorCondition {
    pub fn matches(&self, parameters: &AudioParameterSet) -> bool {
        match self {
            Self::SwitchEquals { name, value } => parameters
                .switches
                .get(name)
                .is_some_and(|actual| actual.eq_ignore_ascii_case(value)),
            Self::ScalarRange { name, min, max } => parameters
                .scalars
                .get(name)
                .is_some_and(|actual| *actual >= *min && *actual <= *max),
        }
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::SwitchEquals { name, value } => {
                validate_symbol("interactive music switch", name)?;
                validate_value("interactive music switch value", value)
            }
            Self::ScalarRange { name, min, max } => {
                validate_symbol("interactive music scalar", name)?;
                if !min.is_finite() || !max.is_finite() || min > max {
                    return Err(format!(
                        "interactive music scalar selector '{}' requires finite min <= max",
                        name
                    ));
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioMusicSelectorSpec {
    pub condition: AudioMusicSelectorCondition,
    pub target_state: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InteractiveMusicGraph {
    pub schema: String,
    pub version: u32,
    pub id: String,
    pub initial_state: String,
    pub stems: Vec<AudioMusicStemSpec>,
    pub states: Vec<AudioMusicStateSpec>,
    pub transitions: Vec<AudioMusicTransitionSpec>,
    /// First matching authored selector wins. Names and values are fully project-owned.
    pub selectors: Vec<AudioMusicSelectorSpec>,
}

impl Default for InteractiveMusicGraph {
    fn default() -> Self {
        Self {
            schema: AUDIO_INTERACTIVE_MUSIC_SCHEMA.to_owned(),
            version: AUDIO_INTERACTIVE_MUSIC_VERSION,
            id: String::new(),
            initial_state: String::new(),
            stems: Vec::new(),
            states: Vec::new(),
            transitions: Vec::new(),
            selectors: Vec::new(),
        }
    }
}

impl InteractiveMusicGraph {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != AUDIO_INTERACTIVE_MUSIC_SCHEMA
            || self.version != AUDIO_INTERACTIVE_MUSIC_VERSION
        {
            return Err(format!(
                "unsupported InteractiveMusicGraph schema='{}' version={}",
                self.schema, self.version
            ));
        }
        validate_symbol("interactive music graph id", &self.id)?;
        validate_symbol("interactive music initial state", &self.initial_state)?;
        if self.stems.is_empty() || self.stems.len() > AUDIO_INTERACTIVE_MUSIC_MAX_STEMS {
            return Err(format!(
                "interactive music graph '{}' requires 1..={} stems",
                self.id, AUDIO_INTERACTIVE_MUSIC_MAX_STEMS
            ));
        }
        if self.states.is_empty() || self.states.len() > AUDIO_INTERACTIVE_MUSIC_MAX_STATES {
            return Err(format!(
                "interactive music graph '{}' requires 1..={} states",
                self.id, AUDIO_INTERACTIVE_MUSIC_MAX_STATES
            ));
        }
        if self.transitions.len() > AUDIO_INTERACTIVE_MUSIC_MAX_TRANSITIONS {
            return Err(format!(
                "interactive music graph '{}' transition count exceeds {}",
                self.id, AUDIO_INTERACTIVE_MUSIC_MAX_TRANSITIONS
            ));
        }
        if self.selectors.len() > AUDIO_INTERACTIVE_MUSIC_MAX_SELECTORS {
            return Err(format!(
                "interactive music graph '{}' selector count exceeds {}",
                self.id, AUDIO_INTERACTIVE_MUSIC_MAX_SELECTORS
            ));
        }

        let mut stems = BTreeSet::<String>::new();
        for stem in &self.stems {
            validate_symbol("interactive music stem", &stem.id)?;
            stem.request.clone().sanitized()?;
            if !stems.insert(stem.id.to_ascii_lowercase()) {
                return Err(format!("duplicate interactive music stem '{}'", stem.id));
            }
        }

        let mut states = BTreeSet::<String>::new();
        for state in &self.states {
            validate_symbol("interactive music state", &state.id)?;
            if !states.insert(state.id.to_ascii_lowercase()) {
                return Err(format!("duplicate interactive music state '{}'", state.id));
            }
            let mut state_stems = BTreeSet::<String>::new();
            for layer in &state.layers {
                validate_symbol("interactive music state stem", &layer.stem)?;
                if !stems.contains(&layer.stem.to_ascii_lowercase()) {
                    return Err(format!(
                        "interactive music state '{}' references unknown stem '{}'",
                        state.id, layer.stem
                    ));
                }
                if !layer.gain.is_finite() || !(0.0..=4.0).contains(&layer.gain) {
                    return Err(format!(
                        "interactive music state '{}' stem '{}' gain must be in [0, 4]",
                        state.id, layer.stem
                    ));
                }
                if !state_stems.insert(layer.stem.to_ascii_lowercase()) {
                    return Err(format!(
                        "interactive music state '{}' repeats stem '{}'",
                        state.id, layer.stem
                    ));
                }
            }
        }
        if !states.contains(&self.initial_state.to_ascii_lowercase()) {
            return Err(format!(
                "interactive music initial state '{}' does not resolve",
                self.initial_state
            ));
        }

        let mut transitions = BTreeSet::<(String, String)>::new();
        for transition in &self.transitions {
            validate_symbol("interactive music transition from", &transition.from)?;
            validate_symbol("interactive music transition to", &transition.to)?;
            let from = transition.from.to_ascii_lowercase();
            let to = transition.to.to_ascii_lowercase();
            if !states.contains(&from) || !states.contains(&to) {
                return Err(format!(
                    "interactive music transition '{} -> {}' references unknown state",
                    transition.from, transition.to
                ));
            }
            if from == to {
                return Err(format!(
                    "interactive music transition '{}' cannot target itself",
                    transition.from
                ));
            }
            if !transitions.insert((from, to)) {
                return Err(format!(
                    "duplicate interactive music transition '{} -> {}'",
                    transition.from, transition.to
                ));
            }
            if let AudioTransportSchedulePoint::Marker { id } = &transition.quantization {
                validate_symbol("interactive music transition marker", id)?;
            }
        }

        for selector in &self.selectors {
            selector.condition.validate()?;
            validate_symbol("interactive music selector target", &selector.target_state)?;
            if !states.contains(&selector.target_state.to_ascii_lowercase()) {
                return Err(format!(
                    "interactive music selector references unknown state '{}'",
                    selector.target_state
                ));
            }
        }
        Ok(())
    }

    pub fn state(&self, id: &str) -> Option<&AudioMusicStateSpec> {
        self.states
            .iter()
            .find(|state| state.id.eq_ignore_ascii_case(id))
    }

    pub fn stem(&self, id: &str) -> Option<&AudioMusicStemSpec> {
        self.stems
            .iter()
            .find(|stem| stem.id.eq_ignore_ascii_case(id))
    }

    pub fn transition(&self, from: &str, to: &str) -> Option<&AudioMusicTransitionSpec> {
        self.transitions.iter().find(|transition| {
            transition.from.eq_ignore_ascii_case(from) && transition.to.eq_ignore_ascii_case(to)
        })
    }

    pub fn selected_state<'a>(&'a self, parameters: &AudioParameterSet) -> Option<&'a str> {
        self.selectors.iter().find_map(|selector| {
            selector
                .condition
                .matches(parameters)
                .then_some(selector.target_state.as_str())
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMusicSessionState {
    pub graph: String,
    pub object_id: u64,
    pub active_state: String,
    pub pending_state: Option<String>,
    pub active_stems: usize,
    pub transition_start_sample: Option<u64>,
    pub transition_complete_sample: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveMusicRuntimeState {
    pub graphs: usize,
    pub sessions: usize,
    pub active_stems: usize,
    pub pending_transitions: usize,
    pub transitions_scheduled: u64,
    pub transitions_completed: u64,
    pub transitions_rejected: u64,
    pub sessions_state: std::collections::BTreeMap<AudioMusicSessionId, AudioMusicSessionState>,
}

fn validate_symbol(kind: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!("{kind} is invalid"));
    }
    Ok(())
}

fn validate_value(kind: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(format!("{kind} is invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioRouteId, AudioVoiceStealRule};

    fn graph() -> InteractiveMusicGraph {
        let mut base = AudioPlayStreamInstanceRequest::new("shared/audio/music/base.ogg");
        base.route = AudioRouteId::new("project.music");
        base.stream.voice_budget = "project.music".to_owned();
        base.stream.concurrency_group = "project.music.stems".to_owned();
        base.stream.concurrency_limit = 8;
        base.stream.steal_rule = AudioVoiceStealRule::RejectNew;
        let mut high = base.clone();
        high.stream.clip.uri = "shared/audio/music/high.ogg".to_owned();
        InteractiveMusicGraph {
            id: "project.score".to_owned(),
            initial_state: "calm".to_owned(),
            stems: vec![
                AudioMusicStemSpec {
                    id: "base".to_owned(),
                    request: base,
                },
                AudioMusicStemSpec {
                    id: "high".to_owned(),
                    request: high,
                },
            ],
            states: vec![
                AudioMusicStateSpec {
                    id: "calm".to_owned(),
                    layers: vec![AudioMusicLayerSpec {
                        stem: "base".to_owned(),
                        gain: 1.0,
                    }],
                },
                AudioMusicStateSpec {
                    id: "intense".to_owned(),
                    layers: vec![
                        AudioMusicLayerSpec {
                            stem: "base".to_owned(),
                            gain: 0.8,
                        },
                        AudioMusicLayerSpec {
                            stem: "high".to_owned(),
                            gain: 1.0,
                        },
                    ],
                },
            ],
            transitions: vec![AudioMusicTransitionSpec {
                from: "calm".to_owned(),
                to: "intense".to_owned(),
                quantization: AudioTransportSchedulePoint::NextBar,
                crossfade_samples: 24_000,
            }],
            selectors: vec![AudioMusicSelectorSpec {
                condition: AudioMusicSelectorCondition::ScalarRange {
                    name: "project.score.intensity".to_owned(),
                    min: 0.5,
                    max: 1.0,
                },
                target_state: "intense".to_owned(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn graph_keeps_project_names_opaque_and_preserves_stream_voice_policy() {
        let graph = graph();
        graph.validate().expect("graph");
        let stem = graph.stem("base").unwrap();
        assert_eq!(stem.request.stream.voice_budget, "project.music");
        assert_eq!(stem.request.stream.concurrency_group, "project.music.stems");
    }

    #[test]
    fn authored_selector_order_and_scalar_condition_are_deterministic() {
        let graph = graph();
        let mut parameters = AudioParameterSet::default();
        parameters
            .set_scalar("project.score.intensity", 0.75)
            .unwrap();
        assert_eq!(graph.selected_state(&parameters), Some("intense"));
    }

    #[test]
    fn graph_rejects_unknown_stem_and_duplicate_transition_pair() {
        let mut bad_stem_graph = graph();
        bad_stem_graph.states[0].layers[0].stem = "missing".to_owned();
        assert!(bad_stem_graph.validate().is_err());
        let mut duplicate_transition_graph = graph();
        duplicate_transition_graph
            .transitions
            .push(duplicate_transition_graph.transitions[0].clone());
        assert!(duplicate_transition_graph.validate().is_err());
    }
}
