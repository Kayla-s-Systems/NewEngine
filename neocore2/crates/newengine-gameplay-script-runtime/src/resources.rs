use std::collections::BTreeMap;

use newengine_gameplay_script_api::{
    GameplayCommandReceipt, GameplayObjectiveState, ScriptedAbilityRequest, ScriptedActionRequest,
    ScriptedStateMachineEventRequest,
};

#[derive(Clone, Debug, PartialEq)]
pub struct GameplayObjectiveRecord {
    pub state: GameplayObjectiveState,
    pub status: Option<String>,
    pub progress: Option<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct GameplayObjectiveBook {
    pub(crate) objectives: BTreeMap<String, GameplayObjectiveRecord>,
}

impl GameplayObjectiveBook {
    #[inline]
    pub fn get(&self, id: &str) -> Option<&GameplayObjectiveRecord> {
        self.objectives.get(id)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &GameplayObjectiveRecord)> {
        self.objectives
            .iter()
            .map(|(id, record)| (id.as_str(), record))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameplayEffectRequest {
    pub effect: String,
    pub position: Option<[f32; 3]>,
    pub source: Option<u64>,
    pub target: Option<u64>,
    pub intensity: f32,
    pub parameters: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default)]
pub struct GameplayEffectBus {
    pub(crate) pending: Vec<GameplayEffectRequest>,
    dropped: u64,
}

impl GameplayEffectBus {
    #[inline]
    pub fn pending(&self) -> &[GameplayEffectRequest] {
        &self.pending
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<GameplayEffectRequest> {
        std::mem::take(&mut self.pending)
    }

    #[inline]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub(crate) fn push_bounded(&mut self, request: GameplayEffectRequest) {
        const MAX_PENDING_EFFECTS: usize = 512;
        if self.pending.len() >= MAX_PENDING_EFFECTS {
            self.pending.remove(0);
            self.dropped = self.dropped.saturating_add(1);
        }
        self.pending.push(request);
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScriptedActionQueue {
    pub(crate) pending: Vec<ScriptedActionRequest>,
}

impl ScriptedActionQueue {
    #[inline]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScriptedAbilityQueue {
    pub(crate) pending: Vec<ScriptedAbilityRequest>,
}

impl ScriptedAbilityQueue {
    #[inline]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptedStateMachineInstance {
    pub instance_id: String,
    pub machine: String,
    pub state: String,
    pub actor: Option<u64>,
    pub target: Option<u64>,
    pub variables: BTreeMap<String, serde_json::Value>,
}

impl ScriptedStateMachineInstance {
    pub fn new(
        instance_id: impl Into<String>,
        machine: impl Into<String>,
        state: impl Into<String>,
    ) -> Self {
        Self {
            instance_id: instance_id.into(),
            machine: machine.into(),
            state: state.into(),
            actor: None,
            target: None,
            variables: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScriptedStateMachineStore {
    pub(crate) instances: BTreeMap<String, ScriptedStateMachineInstance>,
}

impl ScriptedStateMachineStore {
    #[inline]
    pub fn get(&self, instance_id: &str) -> Option<&ScriptedStateMachineInstance> {
        self.instances.get(instance_id)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ScriptedStateMachineInstance)> {
        self.instances
            .iter()
            .map(|(id, instance)| (id.as_str(), instance))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScriptedStateMachineEventQueue {
    pub(crate) pending: Vec<ScriptedStateMachineEventRequest>,
}

impl ScriptedStateMachineEventQueue {
    #[inline]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptedGameplayOutcome {
    pub kind: String,
    pub subject: String,
    pub ok: bool,
    pub message: String,
    pub receipt: Option<GameplayCommandReceipt>,
    pub next_state: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ScriptedGameplayOutcomeBus {
    pending: Vec<ScriptedGameplayOutcome>,
}

impl ScriptedGameplayOutcomeBus {
    #[inline]
    pub fn pending(&self) -> &[ScriptedGameplayOutcome] {
        &self.pending
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<ScriptedGameplayOutcome> {
        std::mem::take(&mut self.pending)
    }

    pub(crate) fn push(&mut self, outcome: ScriptedGameplayOutcome) {
        self.pending.push(outcome);
    }
}
