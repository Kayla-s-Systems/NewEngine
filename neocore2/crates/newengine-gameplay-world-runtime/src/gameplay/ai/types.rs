use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CombatTeam {
    pub id: u32,
}

impl CombatTeam {
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self { id }
    }

    #[inline]
    pub const fn hostile_to(self, other: Self) -> bool {
        self.id != 0 && other.id != 0 && self.id != other.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AIController {
    pub enabled: bool,
    pub decision_interval_seconds: f32,
    pub decision_cooldown_remaining: f32,
}

impl AIController {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            enabled: self.enabled,
            decision_interval_seconds: finite_or(self.decision_interval_seconds, 0.10)
                .clamp(0.016, 10.0),
            decision_cooldown_remaining: finite_non_negative(self.decision_cooldown_remaining, 0.0)
                .clamp(0.0, 10.0),
        }
    }
}

impl Default for AIController {
    fn default() -> Self {
        Self {
            enabled: true,
            decision_interval_seconds: 0.10,
            decision_cooldown_remaining: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerceptionTuning {
    pub sight_range: f32,
    pub field_of_view_degrees: f32,
    pub memory_seconds: f32,
}

impl PerceptionTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            sight_range: finite_non_negative(self.sight_range, 30.0).clamp(0.1, 10_000.0),
            field_of_view_degrees: finite_or(self.field_of_view_degrees, 100.0).clamp(1.0, 360.0),
            memory_seconds: finite_non_negative(self.memory_seconds, 3.0).clamp(0.0, 300.0),
        }
    }
}

impl Default for PerceptionTuning {
    fn default() -> Self {
        Self {
            sight_range: 30.0,
            field_of_view_degrees: 100.0,
            memory_seconds: 3.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PerceptionState {
    pub candidate_target: Option<EntityId>,
    pub visible_target: Option<EntityId>,
    pub candidate_distance: f32,
    pub observation_revision: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TargetMemory {
    pub target: Option<EntityId>,
    pub visible: bool,
    pub last_known_position: Vec3,
    pub seconds_since_seen: f32,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CombatIntentKind {
    #[default]
    Idle,
    Investigate,
    Engage,
}

impl CombatIntentKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Investigate => "investigate",
            Self::Engage => "engage",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CombatIntent {
    pub kind: CombatIntentKind,
    pub target: Option<EntityId>,
    pub target_position: Vec3,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AIPerceptionProbe {
    pub seq: u64,
    pub target: EntityId,
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
    pub sample_dt: f32,
}

#[inline]
pub(super) fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[inline]
pub(super) fn finite_non_negative(value: f32, fallback: f32) -> f32 {
    finite_or(value, fallback).max(0.0)
}
