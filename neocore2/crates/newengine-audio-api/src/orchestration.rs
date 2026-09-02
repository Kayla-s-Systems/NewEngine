use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    sanitize_gain, sanitize_speed, AudioAcousticState, AudioEnvironmentState, AudioMusicSessionId,
    AudioStreamPlayRequest, AudioTransportAction, AudioTransportActionId, AudioTransportConfig,
    AudioTransportSchedulePoint, AudioVoiceBudgetReservation, InteractiveMusicGraph, SoundCueRef,
};

pub const AUDIO_ORCHESTRATION_SCHEMA: &str = "newengine.audio.orchestration.v1";
pub const AUDIO_ORCHESTRATION_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AudioRouteId(pub String);

impl AudioRouteId {
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_symbol("audio route", &self.0)
    }
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct AudioObjectId(pub u64);

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct AudioInstanceId(pub u64);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioParameterSet {
    pub scalars: BTreeMap<String, f32>,
    pub switches: BTreeMap<String, String>,
}

impl AudioParameterSet {
    pub fn sanitized(mut self) -> Self {
        self.scalars.retain(|name, value| {
            validate_symbol("audio parameter", name).is_ok() && value.is_finite()
        });
        for value in self.scalars.values_mut() {
            *value = value.clamp(-1_000_000.0, 1_000_000.0);
        }
        self.switches.retain(|name, value| {
            validate_symbol("audio switch", name).is_ok()
                && !value.trim().is_empty()
                && value.len() <= 256
                && !value.chars().any(char::is_control)
        });
        for value in self.switches.values_mut() {
            *value = value.trim().to_owned();
        }
        self
    }

    pub fn set_scalar(&mut self, name: impl Into<String>, value: f32) -> Result<(), String> {
        let name = name.into();
        validate_symbol("audio parameter", &name)?;
        if !value.is_finite() {
            return Err(format!("audio parameter '{name}' must be finite"));
        }
        self.scalars
            .insert(name, value.clamp(-1_000_000.0, 1_000_000.0));
        Ok(())
    }

    pub fn overlay_from(&mut self, higher_priority: &Self) {
        for (name, value) in &higher_priority.scalars {
            self.scalars.insert(name.clone(), *value);
        }
        for (name, value) in &higher_priority.switches {
            self.switches.insert(name.clone(), value.clone());
        }
    }

    pub fn set_switch(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), String> {
        let name = name.into();
        let value = value.into();
        validate_symbol("audio switch", &name)?;
        let value = value.trim();
        if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err(format!("audio switch '{name}' has an invalid value"));
        }
        self.switches.insert(name, value.to_owned());
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioObjectState {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub gain: f32,
    pub acoustic: AudioAcousticState,
    pub environment: AudioEnvironmentState,
    pub parameters: AudioParameterSet,
}

impl Default for AudioObjectState {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            velocity: [0.0; 3],
            gain: 1.0,
            acoustic: AudioAcousticState::clear(),
            environment: AudioEnvironmentState::clear(),
            parameters: AudioParameterSet::default(),
        }
    }
}

impl AudioObjectState {
    pub fn sanitized(mut self) -> Self {
        self.position = sanitize_vec3(self.position, 1_000_000.0);
        self.velocity = sanitize_vec3(self.velocity, 10_000.0);
        self.gain = sanitize_gain(self.gain);
        self.acoustic = self.acoustic.sanitized();
        self.environment = self.environment.sanitized();
        self.parameters = self.parameters.sanitized();
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioPlayInstanceRequest {
    pub cue: SoundCueRef,
    /// Project-defined logical mix route. Empty means ungrouped/unity gain for migration.
    pub route: AudioRouteId,
    pub tags: Vec<String>,
    pub gain: f32,
    pub pitch: f32,
    pub spatial: bool,
    pub seed: Option<u64>,
    pub parameters: AudioParameterSet,
}

impl Default for AudioPlayInstanceRequest {
    fn default() -> Self {
        Self {
            cue: SoundCueRef::new(String::new()),
            route: AudioRouteId::default(),
            tags: Vec::new(),
            gain: 1.0,
            pitch: 1.0,
            spatial: true,
            seed: None,
            parameters: AudioParameterSet::default(),
        }
    }
}

impl AudioPlayInstanceRequest {
    pub fn new(cue: impl Into<String>) -> Self {
        Self {
            cue: SoundCueRef::new(cue),
            ..Self::default()
        }
    }

    pub fn sanitized(mut self) -> Result<Self, String> {
        self.cue.logical_path = self.cue.logical_path.trim().to_owned();
        if self.cue.logical_path.is_empty() {
            return Err("audio instance requires a non-empty cue reference".to_owned());
        }
        self.route.0 = self.route.0.trim().to_owned();
        if !self.route.0.is_empty() {
            self.route.validate()?;
        }
        self.tags = sanitize_symbols("audio instance tag", self.tags)?;
        self.gain = sanitize_gain(self.gain);
        self.pitch = sanitize_speed(self.pitch);
        self.parameters = self.parameters.sanitized();
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioPlayStreamInstanceRequest {
    pub stream: AudioStreamPlayRequest,
    pub route: AudioRouteId,
    pub tags: Vec<String>,
    /// Additional logical instance gain above the authored stream request gain.
    pub gain: f32,
    /// When true, the AudioObject position overrides the stream request spatial position.
    pub spatial: bool,
    pub parameters: AudioParameterSet,
}

impl Default for AudioPlayStreamInstanceRequest {
    fn default() -> Self {
        Self {
            stream: AudioStreamPlayRequest::default(),
            route: AudioRouteId::default(),
            tags: Vec::new(),
            gain: 1.0,
            spatial: false,
            parameters: AudioParameterSet::default(),
        }
    }
}

impl AudioPlayStreamInstanceRequest {
    pub fn new(uri: impl Into<String>) -> Self {
        let mut request = Self::default();
        request.stream.clip = super::AudioClipRef::new(uri);
        request
    }

    pub fn sanitized(mut self) -> Result<Self, String> {
        self.stream = self.stream.sanitized();
        if self.stream.clip.uri.trim().is_empty() {
            return Err("audio stream instance requires a non-empty stream uri".to_owned());
        }
        self.route.0 = self.route.0.trim().to_owned();
        if !self.route.0.is_empty() {
            self.route.validate()?;
        }
        self.tags = sanitize_symbols("audio stream instance tag", self.tags)?;
        self.gain = sanitize_gain(self.gain);
        self.parameters = self.parameters.sanitized();
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMixBusSpec {
    pub id: AudioRouteId,
    pub parent: Option<AudioRouteId>,
    /// Static authored gain before snapshot contributions.
    pub gain_db: f32,
}

impl Default for AudioMixBusSpec {
    fn default() -> Self {
        Self {
            id: AudioRouteId::default(),
            parent: None,
            gain_db: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMixPatch {
    pub route: AudioRouteId,
    /// Additive dB offset applied to this route and its descendants.
    pub gain_db: f32,
}

impl Default for AudioMixPatch {
    fn default() -> Self {
        Self {
            route: AudioRouteId::default(),
            gain_db: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMixSnapshotSpec {
    pub id: String,
    pub transition_seconds: f32,
    pub patches: Vec<AudioMixPatch>,
}

impl Default for AudioMixSnapshotSpec {
    fn default() -> Self {
        Self {
            id: String::new(),
            transition_seconds: 0.2,
            patches: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioMixGraph {
    pub schema: String,
    pub version: u32,
    pub buses: Vec<AudioMixBusSpec>,
    pub snapshots: Vec<AudioMixSnapshotSpec>,
    /// Project-authored reservations inside the provider physical-voice pool.
    pub voice_budgets: Vec<AudioVoiceBudgetReservation>,
}

impl Default for AudioMixGraph {
    fn default() -> Self {
        Self {
            schema: AUDIO_ORCHESTRATION_SCHEMA.to_owned(),
            version: AUDIO_ORCHESTRATION_VERSION,
            buses: Vec::new(),
            snapshots: Vec::new(),
            voice_budgets: Vec::new(),
        }
    }
}

impl AudioMixGraph {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != AUDIO_ORCHESTRATION_VERSION {
            return Err(format!(
                "unsupported AudioMixGraph version {}",
                self.version
            ));
        }
        if self.schema != AUDIO_ORCHESTRATION_SCHEMA {
            return Err(format!(
                "unsupported AudioMixGraph schema '{}'",
                self.schema
            ));
        }

        let mut voice_budget_ids = BTreeSet::<String>::new();
        for reservation in &self.voice_budgets {
            let reservation = reservation.clone().sanitized()?;
            let key = reservation.id.to_ascii_lowercase();
            if !voice_budget_ids.insert(key) {
                return Err(format!("duplicate audio voice budget '{}'", reservation.id));
            }
        }

        let mut buses = BTreeMap::<&str, Option<&str>>::new();
        for bus in &self.buses {
            bus.id.validate()?;
            if !bus.gain_db.is_finite() || !(-96.0..=24.0).contains(&bus.gain_db) {
                return Err(format!(
                    "audio route '{}' gain_db must be finite and in [-96, 24]",
                    bus.id.0
                ));
            }
            let parent = bus.parent.as_ref().map(|parent| parent.0.as_str());
            if let Some(parent) = parent {
                validate_symbol("audio route parent", parent)?;
                if parent == bus.id.0 {
                    return Err(format!("audio route '{}' cannot parent itself", bus.id.0));
                }
            }
            if buses.insert(bus.id.0.as_str(), parent).is_some() {
                return Err(format!("duplicate audio route '{}'", bus.id.0));
            }
        }
        for (id, parent) in &buses {
            if let Some(parent) = parent {
                if !buses.contains_key(parent) {
                    return Err(format!(
                        "audio route '{id}' references unknown parent '{parent}'"
                    ));
                }
            }
        }
        for id in buses.keys().copied() {
            let mut cursor = Some(id);
            let mut seen = BTreeSet::<&str>::new();
            while let Some(current) = cursor {
                if !seen.insert(current) {
                    return Err(format!("audio mix graph contains a cycle at '{current}'"));
                }
                cursor = buses.get(current).copied().flatten();
            }
        }

        let mut snapshot_ids = BTreeSet::<&str>::new();
        for snapshot in &self.snapshots {
            validate_symbol("audio mix snapshot", &snapshot.id)?;
            if !snapshot_ids.insert(snapshot.id.as_str()) {
                return Err(format!("duplicate audio mix snapshot '{}'", snapshot.id));
            }
            if !snapshot.transition_seconds.is_finite()
                || !(0.0..=60.0).contains(&snapshot.transition_seconds)
            {
                return Err(format!(
                    "audio mix snapshot '{}' transition_seconds must be in [0, 60]",
                    snapshot.id
                ));
            }
            let mut patched = BTreeSet::<&str>::new();
            for patch in &snapshot.patches {
                patch.route.validate()?;
                if !buses.contains_key(patch.route.0.as_str()) {
                    return Err(format!(
                        "audio mix snapshot '{}' references unknown route '{}'",
                        snapshot.id, patch.route.0
                    ));
                }
                if !patched.insert(patch.route.0.as_str()) {
                    return Err(format!(
                        "audio mix snapshot '{}' contains duplicate route '{}'",
                        snapshot.id, patch.route.0
                    ));
                }
                if !patch.gain_db.is_finite() || !(-96.0..=24.0).contains(&patch.gain_db) {
                    return Err(format!(
                        "audio mix snapshot '{}' route '{}' gain_db must be in [-96, 24]",
                        snapshot.id, patch.route.0
                    ));
                }
            }
        }
        Ok(())
    }

    #[inline]
    pub fn contains_route(&self, route: &AudioRouteId) -> bool {
        self.buses.iter().any(|bus| bus.id == *route)
    }

    pub fn snapshot(&self, id: &str) -> Option<&AudioMixSnapshotSpec> {
        self.snapshots.iter().find(|snapshot| snapshot.id == id)
    }

    /// Computes project logical-route gain. Snapshot patches on ancestors affect descendants.
    pub fn effective_gain_db(
        &self,
        route: &AudioRouteId,
        snapshot_weights: &BTreeMap<String, f32>,
    ) -> Result<f32, String> {
        // Runtime installation validates the graph once. Per-frame voice routing must not
        // re-run topology validation for every active logical route.
        let mut ancestry = Vec::<&str>::new();
        let mut cursor = route.0.as_str();
        loop {
            let bus = self
                .buses
                .iter()
                .find(|bus| bus.id.0 == cursor)
                .ok_or_else(|| format!("unknown audio route '{}'", route.0))?;
            ancestry.push(bus.id.0.as_str());
            let Some(parent) = bus.parent.as_ref() else {
                break;
            };
            cursor = parent.0.as_str();
        }

        let mut gain_db = self
            .buses
            .iter()
            .filter(|bus| ancestry.contains(&bus.id.0.as_str()))
            .map(|bus| bus.gain_db)
            .sum::<f32>();
        for (snapshot_id, weight) in snapshot_weights {
            if !weight.is_finite() || *weight <= 0.0 {
                continue;
            }
            let Some(snapshot) = self.snapshot(snapshot_id) else {
                continue;
            };
            let weight = weight.clamp(0.0, 1.0);
            for patch in &snapshot.patches {
                if ancestry.contains(&patch.route.0.as_str()) {
                    gain_db += patch.gain_db * weight;
                }
            }
        }
        Ok(gain_db.clamp(-120.0, 48.0))
    }

    pub fn effective_linear_gain(
        &self,
        route: &AudioRouteId,
        snapshot_weights: &BTreeMap<String, f32>,
    ) -> Result<f32, String> {
        Ok(10.0_f32.powf(self.effective_gain_db(route, snapshot_weights)? / 20.0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioParameterTarget {
    Global,
    Object(AudioObjectId),
    Instance(AudioInstanceId),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum AudioOrchestrationCommand {
    InstallMixGraph {
        graph: AudioMixGraph,
    },
    CreateObject {
        object_id: AudioObjectId,
        state: Box<AudioObjectState>,
    },
    DestroyObject {
        object_id: AudioObjectId,
    },
    UpdateObject {
        object_id: AudioObjectId,
        state: Box<AudioObjectState>,
    },
    Play {
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
    },
    PlayStream {
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: Box<AudioPlayStreamInstanceRequest>,
    },
    StopInstance {
        instance_id: AudioInstanceId,
    },
    StopByTag {
        object_id: AudioObjectId,
        tag: String,
    },
    SetScalar {
        target: AudioParameterTarget,
        name: String,
        value: f32,
    },
    SetSwitch {
        target: AudioParameterTarget,
        name: String,
        value: String,
    },
    ActivateSnapshot {
        snapshot: String,
        weight: f32,
        transition_seconds: Option<f32>,
    },
    DeactivateSnapshot {
        snapshot: String,
        transition_seconds: Option<f32>,
    },
    ConfigureTransport {
        config: AudioTransportConfig,
    },
    ScheduleTransport {
        action_id: AudioTransportActionId,
        when: AudioTransportSchedulePoint,
        action: AudioTransportAction,
    },
    CancelTransportAction {
        action_id: AudioTransportActionId,
    },
    InstallMusicGraph {
        graph: InteractiveMusicGraph,
    },
    CreateMusicSession {
        session_id: AudioMusicSessionId,
        graph: String,
        object_id: AudioObjectId,
    },
    DestroyMusicSession {
        session_id: AudioMusicSessionId,
    },
    RequestMusicState {
        session_id: AudioMusicSessionId,
        state: String,
    },
    SetMusicScalar {
        session_id: AudioMusicSessionId,
        name: String,
        value: f32,
    },
    SetMusicSwitch {
        session_id: AudioMusicSessionId,
        name: String,
        value: String,
    },
}

fn validate_symbol(kind: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{kind} must not be empty"));
    }
    if value.len() > 256 {
        return Err(format!("{kind} exceeds 256 bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{kind} contains control characters"));
    }
    Ok(())
}

fn sanitize_symbols(kind: &str, values: Vec<String>) -> Result<Vec<String>, String> {
    let mut out = BTreeSet::<String>::new();
    for value in values {
        let value = value.trim().to_owned();
        validate_symbol(kind, &value)?;
        out.insert(value);
    }
    Ok(out.into_iter().collect())
}

fn sanitize_vec3(mut value: [f32; 3], absolute_limit: f32) -> [f32; 3] {
    for component in &mut value {
        *component = if component.is_finite() {
            component.clamp(-absolute_limit, absolute_limit)
        } else {
            0.0
        };
    }
    value
}

#[cfg(test)]
mod tests {
    include!("orchestration/tests.rs");
}
