use std::collections::BTreeMap;

use newengine_audio_api::{
    AudioInstanceId, AudioObjectId, AudioPlayInstanceRequest, AudioPlayStreamInstanceRequest,
    AudioTransportAction, AudioTransportActionId, AudioTransportConfig,
    AudioTransportMarkerOccurrence, AudioTransportRuntimeState, AudioTransportSchedulePoint,
    AUDIO_TRANSPORT_MAX_SCHEDULED_ACTIONS,
};

use crate::audio_orchestration::AudioOrchestrationHandle;

/// Project-facing transport capability. It shares the orchestration command queue internally,
/// but exposes only sample-domain clock/scheduling operations.
#[derive(Clone)]
pub struct AudioTransportHandle {
    orchestration: AudioOrchestrationHandle,
}

impl AudioTransportHandle {
    pub(crate) fn new(orchestration: AudioOrchestrationHandle) -> Self {
        Self { orchestration }
    }

    pub fn configure(&self, config: AudioTransportConfig) -> Result<(), String> {
        self.orchestration.configure_transport(config)
    }

    pub fn schedule(
        &self,
        when: AudioTransportSchedulePoint,
        action: AudioTransportAction,
    ) -> Result<AudioTransportActionId, String> {
        self.orchestration.schedule_transport_action(when, action)
    }

    pub fn schedule_play(
        &self,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
        when: AudioTransportSchedulePoint,
    ) -> Result<(AudioInstanceId, AudioTransportActionId), String> {
        self.orchestration.schedule_play(object_id, request, when)
    }

    pub fn schedule_stream(
        &self,
        object_id: AudioObjectId,
        request: AudioPlayStreamInstanceRequest,
        when: AudioTransportSchedulePoint,
    ) -> Result<(AudioInstanceId, AudioTransportActionId), String> {
        self.orchestration.schedule_stream(object_id, request, when)
    }

    pub fn cancel(&self, action_id: AudioTransportActionId) -> Result<(), String> {
        self.orchestration.cancel_transport_action(action_id)
    }

    pub fn drain_markers(&self) -> Vec<AudioTransportMarkerOccurrence> {
        self.orchestration.drain_transport_markers()
    }

    #[inline]
    pub fn dropped_marker_events(&self) -> u64 {
        self.orchestration.dropped_transport_events()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DueTransportAction {
    pub id: AudioTransportActionId,
    pub intended_sample: u64,
    pub dispatch_sample: u64,
    pub lateness_samples: u64,
    pub action: AudioTransportAction,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingTransportAction {
    pub id: AudioTransportActionId,
    pub intended_sample: u64,
    pub action: AudioTransportAction,
}

#[derive(Clone, Debug)]
struct ScheduledAction {
    id: AudioTransportActionId,
    intended_sample: u64,
    action: AudioTransportAction,
}

#[derive(Clone, Debug)]
pub(crate) struct AudioTransportRuntime {
    config: AudioTransportConfig,
    sample: u64,
    fractional_samples: f64,
    scheduled: BTreeMap<(u64, u64), ScheduledAction>,
    emitted_markers: u64,
    executed_actions: u64,
    late_actions: u64,
    max_lateness_samples: u64,
    has_advanced: bool,
}

impl Default for AudioTransportRuntime {
    fn default() -> Self {
        Self::new(
            AudioTransportConfig::default()
                .validate()
                .expect("default transport config"),
        )
    }
}

impl AudioTransportRuntime {
    pub fn new(config: AudioTransportConfig) -> Self {
        Self {
            config,
            sample: 0,
            fractional_samples: 0.0,
            scheduled: BTreeMap::new(),
            emitted_markers: 0,
            executed_actions: 0,
            late_actions: 0,
            max_lateness_samples: 0,
            has_advanced: false,
        }
    }

    #[inline]
    pub fn sample(&self) -> u64 {
        self.sample
    }

    #[inline]
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    pub fn configure(&mut self, config: AudioTransportConfig) -> Result<(), String> {
        let config = config.validate()?;
        if self.sample != 0 || self.has_advanced || !self.scheduled.is_empty() {
            return Err(
                "audio transport cannot be reconfigured after clock start or while actions are pending"
                    .to_owned(),
            );
        }
        self.config = config;
        self.fractional_samples = 0.0;
        Ok(())
    }

    pub fn resolve_schedule_point(
        &self,
        when: &AudioTransportSchedulePoint,
    ) -> Result<u64, String> {
        match when {
            AudioTransportSchedulePoint::Immediate => Ok(self.sample),
            AudioTransportSchedulePoint::AbsoluteSample { sample } => Ok(*sample),
            AudioTransportSchedulePoint::NextBeat => Ok(self
                .config
                .tempo
                .next_beat_sample(self.config.sample_rate, self.sample)),
            AudioTransportSchedulePoint::NextBar => Ok(self
                .config
                .tempo
                .next_bar_sample(self.config.sample_rate, self.sample)),
            AudioTransportSchedulePoint::Marker { id } => self
                .config
                .marker(id)
                .map(|marker| marker.sample)
                .ok_or_else(|| format!("unknown audio transport marker '{id}'")),
        }
    }

    pub fn schedule(
        &mut self,
        id: AudioTransportActionId,
        when: AudioTransportSchedulePoint,
        action: AudioTransportAction,
    ) -> Result<(), String> {
        if id.0 == 0 {
            return Err("audio transport action id must be non-zero".to_owned());
        }
        if self.scheduled.len() >= AUDIO_TRANSPORT_MAX_SCHEDULED_ACTIONS {
            return Err(format!(
                "audio transport scheduled action capacity {} exhausted",
                AUDIO_TRANSPORT_MAX_SCHEDULED_ACTIONS
            ));
        }
        if self.scheduled.values().any(|entry| entry.id == id) {
            return Err(format!("duplicate audio transport action id {}", id.0));
        }
        let action = action.validate()?;
        let intended_sample = self.resolve_schedule_point(&when)?;
        if intended_sample < self.sample {
            return Err(format!(
                "audio transport cannot schedule action {} in the past intended_sample={} current_sample={}",
                id.0, intended_sample, self.sample
            ));
        }
        self.scheduled.insert(
            (intended_sample, id.0),
            ScheduledAction {
                id,
                intended_sample,
                action,
            },
        );
        Ok(())
    }

    pub fn cancel(&mut self, id: AudioTransportActionId) -> bool {
        let key = self
            .scheduled
            .iter()
            .find_map(|(key, entry)| (entry.id == id).then_some(*key));
        key.and_then(|key| self.scheduled.remove(&key)).is_some()
    }

    #[inline]
    pub(crate) fn has_pending_actions(&self) -> bool {
        !self.scheduled.is_empty()
    }

    pub(crate) fn pending_actions(&self) -> Vec<PendingTransportAction> {
        self.scheduled
            .values()
            .map(|entry| PendingTransportAction {
                id: entry.id,
                intended_sample: entry.intended_sample,
                action: entry.action.clone(),
            })
            .collect()
    }

    /// Frame-time adapter used until the native block renderer owns physical advancement.
    /// Fractional samples are carried across calls; the authoritative primitive is
    /// `advance_samples`, which can later be driven directly by rendered PCM block sizes.
    pub fn advance_seconds(
        &mut self,
        dt_seconds: f32,
    ) -> (Vec<AudioTransportMarkerOccurrence>, Vec<DueTransportAction>) {
        let dt = if dt_seconds.is_finite() {
            dt_seconds.max(0.0) as f64
        } else {
            0.0
        };
        let exact = self.fractional_samples + dt * f64::from(self.config.sample_rate);
        let whole = exact.floor().clamp(0.0, u64::MAX as f64) as u64;
        self.fractional_samples = (exact - whole as f64).clamp(0.0, 0.999_999_999_999);
        self.advance_samples(whole)
    }

    /// Advances by an exact number of PCM frames in the transport sample domain.
    pub fn advance_samples(
        &mut self,
        samples: u64,
    ) -> (Vec<AudioTransportMarkerOccurrence>, Vec<DueTransportAction>) {
        let previous = self.sample;
        self.sample = self.sample.saturating_add(samples);

        let markers = self.markers_crossed(previous, self.sample);
        self.emitted_markers = self
            .emitted_markers
            .saturating_add(u64::try_from(markers.len()).unwrap_or(u64::MAX));

        let due_keys = self
            .scheduled
            .range(..=(self.sample, u64::MAX))
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        let mut due = Vec::with_capacity(due_keys.len());
        for key in due_keys {
            let Some(entry) = self.scheduled.remove(&key) else {
                continue;
            };
            let lateness = self.sample.saturating_sub(entry.intended_sample);
            if lateness > 0 {
                self.late_actions = self.late_actions.saturating_add(1);
                self.max_lateness_samples = self.max_lateness_samples.max(lateness);
            }
            self.executed_actions = self.executed_actions.saturating_add(1);
            due.push(DueTransportAction {
                id: entry.id,
                intended_sample: entry.intended_sample,
                dispatch_sample: self.sample,
                lateness_samples: lateness,
                action: entry.action,
            });
        }
        self.has_advanced = true;
        (markers, due)
    }

    fn markers_crossed(&self, previous: u64, current: u64) -> Vec<AudioTransportMarkerOccurrence> {
        self.config
            .markers
            .iter()
            .filter(|marker| {
                (marker.sample > previous && marker.sample <= current)
                    || (!self.has_advanced && previous == 0 && marker.sample == 0)
            })
            .map(|marker| AudioTransportMarkerOccurrence {
                id: marker.id.clone(),
                sample: marker.sample,
                position: self.config.position(marker.sample),
            })
            .collect()
    }

    pub fn snapshot(&self) -> AudioTransportRuntimeState {
        let position = self.config.position(self.sample);
        AudioTransportRuntimeState {
            sample_rate: self.config.sample_rate,
            sample: self.sample,
            beat: position.beat,
            bar: position.bar,
            beat_in_bar: position.beat_in_bar,
            pending_actions: self.scheduled.len(),
            active_transitions: 0,
            emitted_markers: self.emitted_markers,
            executed_actions: self.executed_actions,
            late_actions: self.late_actions,
            max_lateness_samples: self.max_lateness_samples,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_audio_api::{AudioTransportMarker, AudioTransportSchedulePoint};

    #[test]
    fn fractional_sample_carry_is_monotonic_and_drift_free_for_common_frame_step() {
        let mut transport = AudioTransportRuntime::default();
        for _ in 0..60 {
            let _ = transport.advance_seconds(1.0 / 60.0);
        }
        assert_eq!(transport.sample(), 48_000);
        let before = transport.sample();
        let _ = transport.advance_seconds(f32::NAN);
        assert_eq!(transport.sample(), before);
    }

    #[test]
    fn beat_bar_and_marker_schedule_resolve_in_sample_domain() {
        let config = AudioTransportConfig {
            markers: vec![AudioTransportMarker {
                id: "project.drop".to_owned(),
                sample: 72_000,
            }],
            ..Default::default()
        }
        .validate()
        .unwrap();
        let transport = AudioTransportRuntime::new(config);
        assert_eq!(
            transport
                .resolve_schedule_point(&AudioTransportSchedulePoint::NextBeat)
                .unwrap(),
            24_000
        );
        assert_eq!(
            transport
                .resolve_schedule_point(&AudioTransportSchedulePoint::NextBar)
                .unwrap(),
            96_000
        );
        assert_eq!(
            transport
                .resolve_schedule_point(&AudioTransportSchedulePoint::Marker {
                    id: "project.drop".to_owned(),
                })
                .unwrap(),
            72_000
        );
    }
    fn scalar_action(value: f32) -> AudioTransportAction {
        AudioTransportAction::SetScalar {
            target: newengine_audio_api::AudioParameterTarget::Global,
            name: "project.transport.test".to_owned(),
            value,
        }
    }

    #[test]
    fn markers_emit_exactly_once_when_frame_crosses_boundary() {
        let config = AudioTransportConfig {
            markers: vec![
                AudioTransportMarker {
                    id: "zero".to_owned(),
                    sample: 0,
                },
                AudioTransportMarker {
                    id: "cross".to_owned(),
                    sample: 100,
                },
            ],
            ..Default::default()
        }
        .validate()
        .unwrap();
        let mut transport = AudioTransportRuntime::new(config);
        let (first, _) = transport.advance_seconds(50.0 / 48_000.0);
        assert_eq!(
            first.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["zero"]
        );
        let (second, _) = transport.advance_seconds(60.0 / 48_000.0);
        assert_eq!(
            second.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec!["cross"]
        );
        let (third, _) = transport.advance_seconds(60.0 / 48_000.0);
        assert!(third.is_empty());
    }

    #[test]
    fn same_sample_actions_are_dispatched_in_action_id_order() {
        let mut transport = AudioTransportRuntime::default();
        transport
            .schedule(
                AudioTransportActionId(2),
                AudioTransportSchedulePoint::AbsoluteSample { sample: 100 },
                scalar_action(2.0),
            )
            .unwrap();
        transport
            .schedule(
                AudioTransportActionId(1),
                AudioTransportSchedulePoint::AbsoluteSample { sample: 100 },
                scalar_action(1.0),
            )
            .unwrap();
        let (_, due) = transport.advance_samples(101);
        assert_eq!(
            due.iter().map(|item| item.id.0).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(due.iter().all(|item| item.intended_sample == 100));
        assert!(due.iter().all(|item| item.lateness_samples == 1));
    }

    #[test]
    fn scheduling_in_past_is_explicitly_rejected() {
        let mut transport = AudioTransportRuntime::default();
        let _ = transport.advance_seconds(1.0);
        let error = transport
            .schedule(
                AudioTransportActionId(1),
                AudioTransportSchedulePoint::AbsoluteSample { sample: 1 },
                scalar_action(1.0),
            )
            .unwrap_err();
        assert!(error.contains("in the past"));
    }
}
