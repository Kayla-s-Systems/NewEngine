#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FallPresentationBand {
    Low,
    Medium,
    High,
}

#[inline]
pub(super) fn select_fall_presentation_band(
    distance: f32,
    low_available: bool,
    medium_available: bool,
    high_available: bool,
    medium_min_distance: f32,
    high_min_distance: f32,
) -> Option<FallPresentationBand> {
    let distance = if distance.is_finite() {
        distance.max(0.0)
    } else {
        0.0
    };
    let medium_min = if medium_min_distance.is_finite() {
        medium_min_distance.max(0.0)
    } else {
        0.0
    };
    let high_min = if high_min_distance.is_finite() {
        high_min_distance.max(medium_min)
    } else {
        medium_min
    };

    // Severity is authoritative. Missing authored data never substitutes a different animation
    // band: the caller holds the last visible pose instead of presenting a semantically unrelated
    // low/medium/high performance.
    if high_min > 0.0 && distance >= high_min {
        return high_available.then_some(FallPresentationBand::High);
    }
    if medium_min > 0.0 && distance >= medium_min {
        return medium_available.then_some(FallPresentationBand::Medium);
    }
    low_available.then_some(FallPresentationBand::Low)
}

#[derive(Clone, Debug)]
struct ResolvedAnimationSemanticState {
    target: String,
    sequence: u64,
    parameters: serde_json::Value,
}

#[derive(Clone, Debug, Default)]
struct PlayerAnimationSemanticInput {
    states: std::collections::BTreeMap<String, ResolvedAnimationSemanticState>,
    pulses: Vec<ResolvedAnimationSemanticState>,
}

impl PlayerAnimationSemanticInput {
    const MAX_PULSES: usize = 64;

    fn consume(
        &mut self,
        bindings: &std::collections::BTreeMap<String, String>,
        event: &newengine_animation_api::AnimationSemanticEventV1,
    ) -> Result<bool, String> {
        let Some(target) = bindings.get(event.event.as_str()) else {
            return Ok(false);
        };
        let target = target.trim();
        if target.is_empty() {
            return Err(format!(
                "animation event binding target is empty event='{}'",
                event.event
            ));
        }
        let resolved = ResolvedAnimationSemanticState {
            target: target.to_owned(),
            sequence: event.sequence,
            parameters: event.parameters.clone(),
        };
        match event.kind {
            newengine_animation_api::AnimationSemanticEventKind::State => {
                let replace = self
                    .states
                    .get(event.channel.as_str())
                    .map(|previous| event.sequence > previous.sequence)
                    .unwrap_or(true);
                if replace {
                    self.states.insert(event.channel.clone(), resolved);
                }
            }
            newengine_animation_api::AnimationSemanticEventKind::Pulse => {
                if self.pulses.len() >= Self::MAX_PULSES {
                    let overflow = self.pulses.len() + 1 - Self::MAX_PULSES;
                    self.pulses.drain(0..overflow);
                }
                self.pulses.push(resolved);
            }
        }
        Ok(true)
    }

    fn state(&self, channel: &str) -> Option<&ResolvedAnimationSemanticState> {
        self.states.get(channel)
    }

    fn latest_pulse_target(&self, target: &str) -> Option<&ResolvedAnimationSemanticState> {
        self.pulses
            .iter()
            .rev()
            .find(|event| event.target.eq_ignore_ascii_case(target))
    }

    fn discard_pulses_through(&mut self, sequence: u64) {
        self.pulses.retain(|pulse| pulse.sequence > sequence);
    }
}
