#[derive(Clone, Debug)]
struct VoicePolicyInstanceRank {
    instance_id: u64,
    priority: i32,
    audibility: f32,
    distance: f32,
    oldest_voice_id: u64,
}

enum VoicePolicyAdmission {
    Accepted { stolen_instances: Vec<u64> },
    Rejected { reason: String },
}

impl AudioRuntimeState {
    fn concurrency_scope_key(
        policy: &AudioVoicePolicy,
        scope_id: Option<u64>,
    ) -> Result<Option<u64>, String> {
        match policy.scope {
            AudioConcurrencyScope::Global => Ok(None),
            AudioConcurrencyScope::Object => scope_id
                .filter(|id| *id != 0)
                .map(Some)
                .ok_or_else(|| {
                    format!(
                        "audio concurrency group '{}' requires a non-zero object scope_id",
                        policy.group
                    )
                }),
        }
    }

    fn admit_voice_policy(
        &mut self,
        policy: &AudioVoicePolicy,
        scope_id: Option<u64>,
        incoming_instance_id: u64,
    ) -> Result<VoicePolicyAdmission, String> {
        let policy = policy.clone().sanitized()?;
        if policy.group.is_empty() {
            return Ok(VoicePolicyAdmission::Accepted {
                stolen_instances: Vec::new(),
            });
        }
        let scope_key = Self::concurrency_scope_key(&policy, scope_id)?;
        let mut by_instance = BTreeMap::<u64, VoicePolicyInstanceRank>::new();
        for (&voice_id, voice) in &self.voices {
            if voice.policy_instance_id == incoming_instance_id
                || voice.concurrency_group != policy.group
                || voice.concurrency_scope != policy.scope
                || voice.concurrency_scope_id != scope_key
            {
                continue;
            }
            let audibility = self.voice_audibility(voice);
            let distance = voice.distance_to(self.listener);
            by_instance
                .entry(voice.policy_instance_id)
                .and_modify(|rank| {
                    rank.priority = rank.priority.max(voice.priority);
                    rank.audibility = rank.audibility.max(audibility);
                    rank.distance = rank.distance.min(distance);
                    rank.oldest_voice_id = rank.oldest_voice_id.min(voice_id);
                })
                .or_insert(VoicePolicyInstanceRank {
                    instance_id: voice.policy_instance_id,
                    priority: voice.priority,
                    audibility,
                    distance,
                    oldest_voice_id: voice_id,
                });
        }

        let required_steals = by_instance
            .len()
            .saturating_add(1)
            .saturating_sub(policy.limit);
        if required_steals == 0 {
            return Ok(VoicePolicyAdmission::Accepted {
                stolen_instances: Vec::new(),
            });
        }
        if policy.steal_rule == AudioVoiceStealRule::RejectNew {
            return Ok(VoicePolicyAdmission::Rejected {
                reason: format!(
                    "concurrency group '{}' scope={:?} limit={} rejected new instance",
                    policy.group, policy.scope, policy.limit
                ),
            });
        }

        let mut candidates = by_instance.into_values().collect::<Vec<_>>();
        match policy.steal_rule {
            AudioVoiceStealRule::RejectNew => unreachable!("handled above"),
            AudioVoiceStealRule::LowerPriorityThenOldest => {
                candidates.retain(|candidate| candidate.priority <= policy.priority);
                candidates.sort_unstable_by(|a, b| {
                    a.priority
                        .cmp(&b.priority)
                        .then_with(|| a.oldest_voice_id.cmp(&b.oldest_voice_id))
                        .then_with(|| a.instance_id.cmp(&b.instance_id))
                });
            }
            AudioVoiceStealRule::Oldest => {
                candidates.sort_unstable_by(|a, b| {
                    a.oldest_voice_id
                        .cmp(&b.oldest_voice_id)
                        .then_with(|| a.instance_id.cmp(&b.instance_id))
                });
            }
            AudioVoiceStealRule::Quietest => {
                candidates.sort_unstable_by(|a, b| {
                    a.audibility
                        .total_cmp(&b.audibility)
                        .then_with(|| a.priority.cmp(&b.priority))
                        .then_with(|| a.oldest_voice_id.cmp(&b.oldest_voice_id))
                        .then_with(|| a.instance_id.cmp(&b.instance_id))
                });
            }
            AudioVoiceStealRule::Farthest => {
                candidates.sort_unstable_by(|a, b| {
                    b.distance
                        .total_cmp(&a.distance)
                        .then_with(|| a.priority.cmp(&b.priority))
                        .then_with(|| a.oldest_voice_id.cmp(&b.oldest_voice_id))
                        .then_with(|| a.instance_id.cmp(&b.instance_id))
                });
            }
        }
        if candidates.len() < required_steals {
            return Ok(VoicePolicyAdmission::Rejected {
                reason: format!(
                    "concurrency group '{}' scope={:?} limit={} could not steal {} instance(s) under rule {:?}",
                    policy.group,
                    policy.scope,
                    policy.limit,
                    required_steals,
                    policy.steal_rule
                ),
            });
        }

        let stolen_instances = candidates
            .into_iter()
            .take(required_steals)
            .map(|candidate| candidate.instance_id)
            .collect::<Vec<_>>();
        let stolen = stolen_instances.iter().copied().collect::<HashSet<_>>();
        let victim_voices = self
            .voices
            .iter()
            .filter_map(|(voice_id, voice)| {
                stolen
                    .contains(&voice.policy_instance_id)
                    .then_some(*voice_id)
            })
            .collect::<Vec<_>>();
        for voice_id in victim_voices {
            let _ = self.remove_voice(voice_id);
        }
        Ok(VoicePolicyAdmission::Accepted { stolen_instances })
    }

    fn set_voice_budgets(&mut self, request: AudioVoiceBudgetConfig) -> AudioVoiceBudgetAck {
        let request = match request.sanitized() {
            Ok(request) => request,
            Err(error) => {
                return AudioVoiceBudgetAck {
                    accepted: false,
                    max_physical_voices: self.max_physical_voices,
                    reservations: self.voice_budget_reservations.clone(),
                    message: error,
                };
            }
        };
        let reservations = request
            .reservations
            .into_iter()
            .map(|reservation| (reservation.id, reservation.reserved_physical_voices))
            .collect::<BTreeMap<_, _>>();
        let total = reservations.values().copied().sum::<usize>();
        if total > self.max_physical_voices {
            return AudioVoiceBudgetAck {
                accepted: false,
                max_physical_voices: self.max_physical_voices,
                reservations: self.voice_budget_reservations.clone(),
                message: format!(
                    "reserved physical voice total {} exceeds provider budget {}",
                    total, self.max_physical_voices
                ),
            };
        }
        self.voice_budget_reservations = reservations;
        self.rebalance_physical_voices();
        AudioVoiceBudgetAck {
            accepted: true,
            max_physical_voices: self.max_physical_voices,
            reservations: self.voice_budget_reservations.clone(),
            message: String::new(),
        }
    }
}
