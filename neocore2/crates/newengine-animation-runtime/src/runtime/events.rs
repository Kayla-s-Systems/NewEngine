/// Authored key/value metadata attached to a timeline marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationEventParameter {
    pub key: String,
    pub value: String,
}

/// Immutable authored marker carried by an animation clip timeline.
///
/// Tags are semantic engine vocabulary (`foot.left.contact`, `weapon.mag.detach`,
/// `melee.damage_window.begin`, ...). Runtime code must not infer behavior from clip names.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationEvent {
    pub time_seconds: f32,
    pub tag: String,
    pub parameters: Vec<AnimationEventParameter>,
}

impl AnimationEvent {
    #[inline]
    pub fn new(time_seconds: f32, tag: impl Into<String>) -> Self {
        Self {
            time_seconds,
            tag: tag.into(),
            parameters: Vec::new(),
        }
    }

    #[inline]
    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.parameters.push(AnimationEventParameter {
            key: key.into(),
            value: value.into(),
        });
        self
    }
}

/// One concrete occurrence of an authored marker on the unwrapped playback clock.
/// `event_index` addresses `AnimationClip::events` without cloning event payload strings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationEventOccurrence {
    pub event_index: usize,
    pub playback_time_seconds: f32,
    pub loop_index: u64,
}

/// Exactly-once event cursor for monotonically advancing animation playback.
/// The interval policy is open-left/closed-right: `(previous, current]`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationEventCursor {
    last_playback_time_seconds: Option<f32>,
}

impl AnimationEventCursor {
    #[inline]
    pub fn reset(&mut self) {
        self.last_playback_time_seconds = None;
    }

    /// Arms the cursor so an authored marker at `t=0` is emitted on the next advance.
    #[inline]
    pub fn restart(&mut self) {
        self.last_playback_time_seconds = Some(-f32::EPSILON);
    }

    #[inline]
    pub fn seek(&mut self, playback_time_seconds: f32) -> Result<(), String> {
        if !playback_time_seconds.is_finite() || playback_time_seconds < 0.0 {
            return Err(format!(
                "animation event seek time is invalid time={playback_time_seconds}"
            ));
        }
        self.last_playback_time_seconds = Some(playback_time_seconds);
        Ok(())
    }

    #[inline]
    pub fn last_playback_time_seconds(&self) -> Option<f32> {
        self.last_playback_time_seconds
    }

    pub fn advance(
        &mut self,
        clip: &AnimationClip,
        playback_time_seconds: f32,
        out: &mut Vec<AnimationEventOccurrence>,
    ) -> Result<usize, String> {
        if !playback_time_seconds.is_finite() || playback_time_seconds < 0.0 {
            return Err(format!(
                "animation event playback time is invalid clip='{}' time={playback_time_seconds}",
                clip.name
            ));
        }
        let Some(previous) = self.last_playback_time_seconds else {
            self.last_playback_time_seconds = Some(playback_time_seconds);
            return Ok(0);
        };
        self.last_playback_time_seconds = Some(playback_time_seconds);
        if playback_time_seconds < previous {
            // Backward time is an explicit seek/state restart. Do not synthesize history.
            return Ok(0);
        }
        clip.collect_events_between(previous, playback_time_seconds, out)
    }
}

impl AnimationClip {
    pub fn validate_events(&self) -> Result<(), String> {
        use std::collections::HashSet;

        let duration = self.duration_seconds;
        if !duration.is_finite() || duration <= 0.0 {
            return Err(format!(
                "animation event validation requires valid clip duration clip='{}' duration={duration}",
                self.name
            ));
        }
        let mut previous = -1.0_f32;
        for (event_index, event) in self.events.iter().enumerate() {
            if !event.time_seconds.is_finite() || event.time_seconds < 0.0 {
                return Err(format!(
                    "animation event time is invalid clip='{}' event={} time={}",
                    self.name, event_index, event.time_seconds
                ));
            }
            let upper_valid = if self.looped {
                event.time_seconds < duration
            } else {
                event.time_seconds <= duration
            };
            if !upper_valid {
                return Err(format!(
                    "animation event is outside clip duration clip='{}' event={} time={} duration={} looped={}",
                    self.name, event_index, event.time_seconds, duration, self.looped
                ));
            }
            if event.time_seconds < previous {
                return Err(format!(
                    "animation events must be sorted by time clip='{}' event={} time={} previous={previous}",
                    self.name, event_index, event.time_seconds
                ));
            }
            previous = event.time_seconds;
            if event.tag.trim().is_empty() {
                return Err(format!(
                    "animation event tag is empty clip='{}' event={event_index}",
                    self.name
                ));
            }
            let mut keys = HashSet::with_capacity(event.parameters.len());
            for parameter in &event.parameters {
                let key = parameter.key.trim();
                if key.is_empty() {
                    return Err(format!(
                        "animation event parameter key is empty clip='{}' event={event_index}",
                        self.name
                    ));
                }
                if !keys.insert(key.to_ascii_lowercase()) {
                    return Err(format!(
                        "animation event parameter key is duplicated clip='{}' event={} key='{}'",
                        self.name, event_index, parameter.key
                    ));
                }
            }
        }
        Ok(())
    }

    /// Appends all authored markers crossed by `(previous, current]` on the unwrapped clock.
    pub fn collect_events_between(
        &self,
        previous_playback_time_seconds: f32,
        current_playback_time_seconds: f32,
        out: &mut Vec<AnimationEventOccurrence>,
    ) -> Result<usize, String> {
        const MAX_EMISSIONS_PER_ADVANCE: usize = 4096;
        if !previous_playback_time_seconds.is_finite()
            || !current_playback_time_seconds.is_finite()
            || previous_playback_time_seconds < -f32::EPSILON
            || current_playback_time_seconds < 0.0
            || current_playback_time_seconds < previous_playback_time_seconds
        {
            return Err(format!(
                "animation event interval is invalid clip='{}' previous={} current={}",
                self.name, previous_playback_time_seconds, current_playback_time_seconds
            ));
        }
        if self.events.is_empty() || current_playback_time_seconds == previous_playback_time_seconds {
            return Ok(0);
        }
        self.validate_events()?;
        let initial_len = out.len();
        if !self.looped {
            for (event_index, event) in self.events.iter().enumerate() {
                if event.time_seconds > previous_playback_time_seconds
                    && event.time_seconds <= current_playback_time_seconds
                {
                    out.push(AnimationEventOccurrence {
                        event_index,
                        playback_time_seconds: event.time_seconds,
                        loop_index: 0,
                    });
                }
            }
            return Ok(out.len() - initial_len);
        }

        let duration = self.duration_seconds;
        for (event_index, event) in self.events.iter().enumerate() {
            let base = event.time_seconds;
            let first_loop = if previous_playback_time_seconds < base {
                0
            } else {
                (((previous_playback_time_seconds - base) / duration).floor() as u64)
                    .saturating_add(1)
            };
            let mut loop_index = first_loop;
            loop {
                let occurrence = base + duration * loop_index as f32;
                if occurrence > current_playback_time_seconds {
                    break;
                }
                if occurrence > previous_playback_time_seconds {
                    if out.len() - initial_len >= MAX_EMISSIONS_PER_ADVANCE {
                        return Err(format!(
                            "animation event emission budget exceeded clip='{}' previous={} current={} limit={MAX_EMISSIONS_PER_ADVANCE}",
                            self.name, previous_playback_time_seconds, current_playback_time_seconds
                        ));
                    }
                    out.push(AnimationEventOccurrence {
                        event_index,
                        playback_time_seconds: occurrence,
                        loop_index,
                    });
                }
                loop_index = loop_index.saturating_add(1);
                if loop_index == u64::MAX {
                    break;
                }
            }
        }
        out[initial_len..].sort_by(|a, b| {
            a.playback_time_seconds
                .total_cmp(&b.playback_time_seconds)
                .then_with(|| a.event_index.cmp(&b.event_index))
        });
        Ok(out.len() - initial_len)
    }
}
