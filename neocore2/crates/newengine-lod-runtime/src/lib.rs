#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned hierarchical LOD/significance control plane.
//!
//! Visibility produces provider-neutral significance. This crate consumes that
//! signal plus authored hierarchy/LOD capacity and turns it into stable detail
//! decisions. It owns no occlusion queries, renderer handles, meshes or assets.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use newengine_visibility_runtime::VisibilityPlanEntry;

pub const MAX_AUTHORED_LODS: u8 = 16;
pub const MAX_TRANSITION_BUDGET: usize = 65_536;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignificanceTier {
    Critical,
    High,
    #[default]
    Medium,
    Low,
    Background,
}

impl SignificanceTier {
    #[inline]
    pub const fn update_interval_frames(self) -> u8 {
        match self {
            Self::Critical | Self::High => 1,
            Self::Medium => 2,
            Self::Low => 4,
            Self::Background => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LodTransition {
    #[default]
    None,
    Upgrade,
    Downgrade,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodSubjectSignal {
    pub subject_id: u64,
    pub parent_id: Option<u64>,
    /// Provider-neutral visibility significance in [0,1].
    pub visibility_significance: f32,
    /// Number of authored detail levels available to the subject. LOD 0 is the
    /// highest detail. Values are sanitized to [1, MAX_AUTHORED_LODS].
    pub authored_lod_count: u8,
    /// Semantic bias for mission/gameplay-important objects. This is additive and
    /// deliberately independent from visibility backend confidence.
    pub significance_bias: f32,
    /// Optional hard detail selection for continuity-critical content.
    pub forced_lod: Option<u8>,
}

impl LodSubjectSignal {
    #[inline]
    pub fn from_visibility(entry: &VisibilityPlanEntry, authored_lod_count: u8) -> Self {
        Self {
            subject_id: entry.subject_id,
            parent_id: None,
            visibility_significance: entry.significance,
            authored_lod_count,
            significance_bias: 0.0,
            forced_lod: None,
        }
    }

    fn sanitized(self) -> Self {
        Self {
            subject_id: self.subject_id,
            parent_id: self.parent_id.filter(|parent| *parent != self.subject_id),
            visibility_significance: finite01(self.visibility_significance),
            authored_lod_count: self.authored_lod_count.clamp(1, MAX_AUTHORED_LODS),
            significance_bias: if self.significance_bias.is_finite() {
                self.significance_bias.clamp(-1.0, 1.0)
            } else {
                0.0
            },
            forced_lod: self
                .forced_lod
                .map(|lod| lod.min(self.authored_lod_count.saturating_sub(1))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HierarchicalLodConfig {
    /// Multiplier derived from authored/user quality policy. >1 preserves higher
    /// detail farther away; <1 moves subjects toward cheaper LODs.
    pub quality_scale: f32,
    /// Parent significance inherited by children. 0 disables hierarchy influence.
    pub parent_inheritance: f32,
    /// Significance dead-band around LOD boundaries.
    pub hysteresis: f32,
    /// Maximum non-forced LOD transitions admitted in one frame.
    pub transition_budget: usize,
    /// Maximum subjects allowed to target LOD0. `usize::MAX` disables the cap.
    pub lod0_budget: usize,
    /// State not observed for this many frames is removed.
    pub state_retention_frames: u64,
}

impl Default for HierarchicalLodConfig {
    fn default() -> Self {
        Self {
            quality_scale: 1.0,
            parent_inheritance: 0.80,
            hysteresis: 0.04,
            transition_budget: 256,
            lod0_budget: 2_048,
            state_retention_frames: 300,
        }
    }
}

impl HierarchicalLodConfig {
    fn sanitized(self) -> Self {
        Self {
            quality_scale: if self.quality_scale.is_finite() {
                self.quality_scale.clamp(0.25, 4.0)
            } else {
                1.0
            },
            parent_inheritance: if self.parent_inheritance.is_finite() {
                self.parent_inheritance.clamp(0.0, 1.0)
            } else {
                0.80
            },
            hysteresis: if self.hysteresis.is_finite() {
                self.hysteresis.clamp(0.0, 0.24)
            } else {
                0.04
            },
            transition_budget: self.transition_budget.min(MAX_TRANSITION_BUDGET),
            lod0_budget: self.lod0_budget,
            state_retention_frames: self.state_retention_frames.max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodPlanEntry {
    pub subject_id: u64,
    pub parent_id: Option<u64>,
    pub own_significance: f32,
    pub inherited_significance: f32,
    pub effective_significance: f32,
    pub tier: SignificanceTier,
    pub lod_index: u8,
    pub previous_lod: u8,
    pub authored_lod_count: u8,
    pub transition: LodTransition,
    pub transition_deferred: bool,
    pub update_interval_frames: u8,
    /// Generic 0..1000 pressure signal that streaming/residency may consume.
    pub detail_pressure: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LodPlanStats {
    pub subjects: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub background: usize,
    pub parent_links: usize,
    pub missing_parents: usize,
    pub hierarchy_cycles: usize,
    pub forced_lods: usize,
    pub lod0_requested: usize,
    pub lod0_budget_demotions: usize,
    pub transitions_requested: usize,
    pub transitions_applied: usize,
    pub transitions_deferred: usize,
    pub upgrades: usize,
    pub downgrades: usize,
    pub pruned_states: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HierarchicalLodPlan {
    pub frame: u64,
    pub entries: Vec<LodPlanEntry>,
    pub stats: LodPlanStats,
}

impl HierarchicalLodPlan {
    #[inline]
    pub fn entry(&self, subject_id: u64) -> Option<&LodPlanEntry> {
        self.entries
            .binary_search_by_key(&subject_id, |entry| entry.subject_id)
            .ok()
            .map(|index| &self.entries[index])
    }
}

#[derive(Clone, Copy, Debug)]
struct SubjectState {
    last_seen_frame: u64,
    lod_index: u8,
}

#[derive(Clone, Copy, Debug)]
struct WorkingSubject {
    signal: LodSubjectSignal,
    own_significance: f32,
    inherited_significance: f32,
    effective_significance: f32,
    target_lod: u8,
    previous_lod: u8,
    initialized: bool,
    transition_deferred: bool,
}

#[derive(Clone, Copy, Debug)]
struct TransitionRequest {
    subject_id: u64,
    transition: LodTransition,
    significance: f32,
}

#[derive(Clone, Debug, Default)]
pub struct HierarchicalLodControlPlane {
    states: BTreeMap<u64, SubjectState>,
}

impl HierarchicalLodControlPlane {
    #[inline]
    pub fn clear(&mut self) {
        self.states.clear();
    }

    #[inline]
    pub fn tracked_subjects(&self) -> usize {
        self.states.len()
    }

    pub fn evaluate(
        &mut self,
        frame: u64,
        signals: &[LodSubjectSignal],
        config: HierarchicalLodConfig,
    ) -> HierarchicalLodPlan {
        let config = config.sanitized();
        let mut stats = LodPlanStats {
            subjects: signals.len(),
            ..LodPlanStats::default()
        };

        let before = self.states.len();
        self.states.retain(|_, state| {
            frame.saturating_sub(state.last_seen_frame) <= config.state_retention_frames
        });
        stats.pruned_states = before.saturating_sub(self.states.len());

        // Stable identity de-duplicates accidental repeated producers and makes
        // outcomes independent from ECS/archetype traversal order.
        let mut ordered = BTreeMap::<u64, LodSubjectSignal>::new();
        for signal in signals.iter().copied() {
            ordered.insert(signal.subject_id, signal.sanitized());
        }
        stats.subjects = ordered.len();

        let effective = resolve_hierarchy_significance(
            &ordered,
            config.parent_inheritance,
            config.quality_scale,
            &mut stats,
        );
        let mut working = BTreeMap::<u64, WorkingSubject>::new();
        for (&subject_id, &signal) in &ordered {
            let own_significance = biased_significance(
                signal.visibility_significance,
                signal.significance_bias,
                config.quality_scale,
            );
            let inherited_significance = effective
                .get(&subject_id)
                .copied()
                .unwrap_or(own_significance)
                .saturating_sub_f32(own_significance);
            let effective_significance = effective
                .get(&subject_id)
                .copied()
                .unwrap_or(own_significance)
                .max(own_significance)
                .clamp(0.0, 1.0);

            let existing = self.states.get(&subject_id).copied();
            let initialized = existing.is_some();
            let previous_lod = existing
                .map(|state| state.lod_index.min(signal.authored_lod_count - 1))
                .unwrap_or_else(|| {
                    raw_lod_for_significance(effective_significance, signal.authored_lod_count)
                });
            let mut target_lod = if let Some(forced) = signal.forced_lod {
                stats.forced_lods += 1;
                forced.min(signal.authored_lod_count - 1)
            } else {
                hysteretic_target_lod(
                    previous_lod,
                    effective_significance,
                    signal.authored_lod_count,
                    config.hysteresis,
                )
            };
            target_lod = target_lod.min(signal.authored_lod_count - 1);
            working.insert(
                subject_id,
                WorkingSubject {
                    signal,
                    own_significance,
                    inherited_significance,
                    effective_significance,
                    target_lod,
                    previous_lod,
                    initialized,
                    transition_deferred: false,
                },
            );
        }

        enforce_lod0_budget(&mut working, config.lod0_budget, &mut stats);

        let mut requests = Vec::<TransitionRequest>::new();
        for (&subject_id, item) in &working {
            if !item.initialized
                || item.signal.forced_lod.is_some()
                || item.target_lod == item.previous_lod
            {
                continue;
            }
            let transition = if item.target_lod < item.previous_lod {
                LodTransition::Upgrade
            } else {
                LodTransition::Downgrade
            };
            requests.push(TransitionRequest {
                subject_id,
                transition,
                significance: item.effective_significance,
            });
        }
        stats.transitions_requested = requests.len();
        requests.sort_by(|a, b| {
            transition_rank(a.transition)
                .cmp(&transition_rank(b.transition))
                .then_with(|| match a.transition {
                    LodTransition::Upgrade => b
                        .significance
                        .partial_cmp(&a.significance)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    LodTransition::Downgrade => a
                        .significance
                        .partial_cmp(&b.significance)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    LodTransition::None => std::cmp::Ordering::Equal,
                })
                .then_with(|| a.subject_id.cmp(&b.subject_id))
        });
        let admitted = requests
            .iter()
            .take(config.transition_budget)
            .map(|request| request.subject_id)
            .collect::<BTreeSet<_>>();
        stats.transitions_applied = admitted.len();
        stats.transitions_deferred = requests.len().saturating_sub(admitted.len());

        let mut entries = Vec::with_capacity(working.len());
        for (subject_id, mut item) in working {
            let forced = item.signal.forced_lod.is_some();
            let final_lod = if !item.initialized || forced || item.target_lod == item.previous_lod {
                item.target_lod
            } else if admitted.contains(&subject_id) {
                // Limit ordinary transitions to one authored level per frame. This
                // avoids a 0->3 or 3->0 visual discontinuity after a pressure spike.
                if item.target_lod < item.previous_lod {
                    item.previous_lod.saturating_sub(1)
                } else {
                    item.previous_lod
                        .saturating_add(1)
                        .min(item.signal.authored_lod_count - 1)
                }
            } else {
                item.transition_deferred = true;
                item.previous_lod
            };
            let transition = if final_lod < item.previous_lod {
                stats.upgrades += 1;
                LodTransition::Upgrade
            } else if final_lod > item.previous_lod {
                stats.downgrades += 1;
                LodTransition::Downgrade
            } else {
                LodTransition::None
            };
            let tier = tier_for_significance(item.effective_significance);
            match tier {
                SignificanceTier::Critical => stats.critical += 1,
                SignificanceTier::High => stats.high += 1,
                SignificanceTier::Medium => stats.medium += 1,
                SignificanceTier::Low => stats.low += 1,
                SignificanceTier::Background => stats.background += 1,
            }
            self.states.insert(
                subject_id,
                SubjectState {
                    last_seen_frame: frame,
                    lod_index: final_lod,
                },
            );
            entries.push(LodPlanEntry {
                subject_id,
                parent_id: item.signal.parent_id,
                own_significance: item.own_significance,
                inherited_significance: item.inherited_significance,
                effective_significance: item.effective_significance,
                tier,
                lod_index: final_lod,
                previous_lod: item.previous_lod,
                authored_lod_count: item.signal.authored_lod_count,
                transition,
                transition_deferred: item.transition_deferred,
                update_interval_frames: tier.update_interval_frames(),
                detail_pressure: detail_pressure(
                    item.effective_significance,
                    final_lod,
                    item.signal.authored_lod_count,
                ),
            });
        }
        entries.sort_by_key(|entry| entry.subject_id);
        HierarchicalLodPlan {
            frame,
            entries,
            stats,
        }
    }
}

fn resolve_hierarchy_significance(
    subjects: &BTreeMap<u64, LodSubjectSignal>,
    inheritance: f32,
    quality_scale: f32,
    stats: &mut LodPlanStats,
) -> BTreeMap<u64, f32> {
    let mut indegree = BTreeMap::<u64, usize>::new();
    let mut children = BTreeMap::<u64, Vec<u64>>::new();
    let mut effective = BTreeMap::<u64, f32>::new();

    for (&id, signal) in subjects {
        effective.insert(
            id,
            biased_significance(
                signal.visibility_significance,
                signal.significance_bias,
                quality_scale,
            ),
        );
        indegree.insert(id, 0);
    }
    for (&id, signal) in subjects {
        let Some(parent) = signal.parent_id else {
            continue;
        };
        if subjects.contains_key(&parent) {
            stats.parent_links += 1;
            *indegree.entry(id).or_default() += 1;
            children.entry(parent).or_default().push(id);
        } else {
            stats.missing_parents += 1;
        }
    }
    for list in children.values_mut() {
        list.sort_unstable();
    }

    let mut ready = VecDeque::new();
    for (&id, &degree) in &indegree {
        if degree == 0 {
            ready.push_back(id);
        }
    }
    let mut visited = BTreeSet::new();
    while let Some(id) = ready.pop_front() {
        visited.insert(id);
        let parent_sig = effective.get(&id).copied().unwrap_or(0.0);
        if let Some(child_ids) = children.get(&id) {
            for &child in child_ids {
                let inherited = parent_sig * inheritance;
                let own = effective.get(&child).copied().unwrap_or(0.0);
                effective.insert(child, own.max(inherited).clamp(0.0, 1.0));
                if let Some(degree) = indegree.get_mut(&child) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        ready.push_back(child);
                    }
                }
            }
        }
    }

    let cyclic = subjects.len().saturating_sub(visited.len());
    if cyclic > 0 {
        // Cyclic parent edges are invalid hierarchy input. Keep each cyclic node's
        // own significance instead of recursively amplifying a malformed loop.
        stats.hierarchy_cycles = cyclic;
        for (&id, signal) in subjects {
            if !visited.contains(&id) {
                effective.insert(
                    id,
                    biased_significance(
                        signal.visibility_significance,
                        signal.significance_bias,
                        quality_scale,
                    ),
                );
            }
        }
    }
    effective
}

fn enforce_lod0_budget(
    working: &mut BTreeMap<u64, WorkingSubject>,
    budget: usize,
    stats: &mut LodPlanStats,
) {
    let mut requested = working
        .iter()
        .filter_map(|(&id, item)| {
            (item.target_lod == 0 && item.signal.authored_lod_count > 1).then_some((
                id,
                item.effective_significance,
                item.signal.forced_lod == Some(0),
            ))
        })
        .collect::<Vec<_>>();
    stats.lod0_requested = requested.len();
    if budget == usize::MAX || requested.len() <= budget {
        return;
    }
    requested.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.0.cmp(&b.0))
    });
    let forced_count = requested
        .iter()
        .take_while(|(_, _, forced)| *forced)
        .count();
    let keep = budget.max(forced_count);
    for &(id, _, forced) in requested.iter().skip(keep) {
        if forced {
            continue;
        }
        if let Some(item) = working.get_mut(&id) {
            item.target_lod = 1.min(item.signal.authored_lod_count - 1);
            stats.lod0_budget_demotions += 1;
        }
    }
}

#[inline]
fn transition_rank(transition: LodTransition) -> u8 {
    match transition {
        LodTransition::Upgrade => 0,
        LodTransition::Downgrade => 1,
        LodTransition::None => 2,
    }
}

#[inline]
fn tier_for_significance(significance: f32) -> SignificanceTier {
    let significance = finite01(significance);
    if significance >= 0.85 {
        SignificanceTier::Critical
    } else if significance >= 0.60 {
        SignificanceTier::High
    } else if significance >= 0.30 {
        SignificanceTier::Medium
    } else if significance >= 0.10 {
        SignificanceTier::Low
    } else {
        SignificanceTier::Background
    }
}

#[inline]
fn biased_significance(raw: f32, bias: f32, quality_scale: f32) -> f32 {
    let base = finite01(raw + bias);
    (base * quality_scale.sqrt()).clamp(0.0, 1.0)
}

#[inline]
fn raw_lod_for_significance(significance: f32, lod_count: u8) -> u8 {
    let lod_count = lod_count.clamp(1, MAX_AUTHORED_LODS);
    if lod_count == 1 {
        return 0;
    }
    let scaled = (1.0 - finite01(significance)) * f32::from(lod_count);
    (scaled.floor() as u8).min(lod_count - 1)
}

fn hysteretic_target_lod(current: u8, significance: f32, lod_count: u8, hysteresis: f32) -> u8 {
    let lod_count = lod_count.clamp(1, MAX_AUTHORED_LODS);
    let current = current.min(lod_count - 1);
    if lod_count == 1 {
        return 0;
    }
    let raw = raw_lod_for_significance(significance, lod_count);
    if raw == current {
        return current;
    }
    let significance = finite01(significance);
    if raw < current {
        // Boundary between current-1 and current.
        let boundary = 1.0 - f32::from(current) / f32::from(lod_count);
        if significance >= boundary + hysteresis {
            raw
        } else {
            current
        }
    } else {
        // Boundary between current and current+1.
        let boundary = 1.0 - f32::from(current + 1) / f32::from(lod_count);
        if significance <= boundary - hysteresis {
            raw
        } else {
            current
        }
    }
}

#[inline]
fn detail_pressure(significance: f32, lod_index: u8, lod_count: u8) -> u16 {
    let lod_count = lod_count.max(1);
    let detail = 1.0 - f32::from(lod_index) / f32::from(lod_count);
    ((finite01(significance) * 0.75 + detail * 0.25) * 1000.0)
        .round()
        .clamp(0.0, 1000.0) as u16
}

#[inline]
fn finite01(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

trait SaturatingSubF32 {
    fn saturating_sub_f32(self, rhs: f32) -> f32;
}

impl SaturatingSubF32 for f32 {
    #[inline]
    fn saturating_sub_f32(self, rhs: f32) -> f32 {
        (self - rhs).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_visibility_runtime::{VisibilityDecision, VisibilityPlanEntry};

    fn signal(id: u64, significance: f32, lods: u8) -> LodSubjectSignal {
        LodSubjectSignal {
            subject_id: id,
            parent_id: None,
            visibility_significance: significance,
            authored_lod_count: lods,
            significance_bias: 0.0,
            forced_lod: None,
        }
    }

    #[test]
    fn high_and_low_significance_select_opposite_lod_ends() {
        let mut control = HierarchicalLodControlPlane::default();
        let plan = control.evaluate(
            1,
            &[signal(1, 0.99, 4), signal(2, 0.01, 4)],
            HierarchicalLodConfig::default(),
        );
        assert_eq!(plan.entry(1).unwrap().lod_index, 0);
        assert_eq!(plan.entry(2).unwrap().lod_index, 3);
        assert_eq!(plan.entry(1).unwrap().tier, SignificanceTier::Critical);
        assert_eq!(plan.entry(2).unwrap().tier, SignificanceTier::Background);
    }

    #[test]
    fn parent_significance_lifts_child_without_visibility_queries() {
        let mut control = HierarchicalLodControlPlane::default();
        let mut child = signal(2, 0.05, 4);
        child.parent_id = Some(1);
        let plan = control.evaluate(
            1,
            &[child, signal(1, 0.95, 4)],
            HierarchicalLodConfig {
                parent_inheritance: 0.8,
                ..HierarchicalLodConfig::default()
            },
        );
        let child = plan.entry(2).unwrap();
        assert!(child.effective_significance >= 0.75);
        assert!(child.inherited_significance > 0.60);
        assert!(child.lod_index <= 1);
    }

    #[test]
    fn quality_scale_applies_to_parent_inheritance() {
        let mut child = signal(2, 0.05, 4);
        child.parent_id = Some(1);
        let mut high = HierarchicalLodControlPlane::default();
        let high_plan = high.evaluate(
            1,
            &[signal(1, 0.95, 4), child],
            HierarchicalLodConfig {
                quality_scale: 1.0,
                parent_inheritance: 0.8,
                ..HierarchicalLodConfig::default()
            },
        );
        let mut low = HierarchicalLodControlPlane::default();
        let low_plan = low.evaluate(
            1,
            &[signal(1, 0.95, 4), child],
            HierarchicalLodConfig {
                quality_scale: 0.5,
                parent_inheritance: 0.8,
                ..HierarchicalLodConfig::default()
            },
        );
        assert!(
            low_plan.entry(2).unwrap().effective_significance
                < high_plan.entry(2).unwrap().effective_significance
        );
    }

    #[test]
    fn hierarchy_cycle_fails_to_own_significance_deterministically() {
        let mut a = signal(1, 0.2, 4);
        let mut b = signal(2, 0.9, 4);
        a.parent_id = Some(2);
        b.parent_id = Some(1);
        let mut control = HierarchicalLodControlPlane::default();
        let first = control.evaluate(1, &[a, b], HierarchicalLodConfig::default());
        control.clear();
        let second = control.evaluate(1, &[b, a], HierarchicalLodConfig::default());
        assert_eq!(first.entries, second.entries);
        assert_eq!(first.stats.hierarchy_cycles, 2);
        assert!((first.entry(1).unwrap().effective_significance - 0.2).abs() < 0.001);
        assert!((first.entry(2).unwrap().effective_significance - 0.9).abs() < 0.001);
    }

    #[test]
    fn hysteresis_prevents_boundary_thrashing() {
        let mut control = HierarchicalLodControlPlane::default();
        let cfg = HierarchicalLodConfig {
            hysteresis: 0.05,
            transition_budget: 16,
            ..HierarchicalLodConfig::default()
        };
        let initial = control.evaluate(1, &[signal(7, 0.76, 4)], cfg);
        assert_eq!(initial.entry(7).unwrap().lod_index, 0);
        let near_boundary = control.evaluate(2, &[signal(7, 0.73, 4)], cfg);
        assert_eq!(near_boundary.entry(7).unwrap().lod_index, 0);
        let clear_cross = control.evaluate(3, &[signal(7, 0.60, 4)], cfg);
        assert_eq!(clear_cross.entry(7).unwrap().lod_index, 1);
    }

    #[test]
    fn ordinary_transitions_are_bounded_and_move_one_level_per_frame() {
        let mut control = HierarchicalLodControlPlane::default();
        let cfg = HierarchicalLodConfig {
            transition_budget: 1,
            hysteresis: 0.0,
            ..HierarchicalLodConfig::default()
        };
        let _ = control.evaluate(1, &[signal(1, 0.99, 4), signal(2, 0.99, 4)], cfg);
        let plan = control.evaluate(2, &[signal(1, 0.01, 4), signal(2, 0.01, 4)], cfg);
        assert_eq!(plan.stats.transitions_requested, 2);
        assert_eq!(plan.stats.transitions_applied, 1);
        assert_eq!(plan.stats.transitions_deferred, 1);
        let changed = plan
            .entries
            .iter()
            .filter(|entry| entry.transition != LodTransition::None)
            .count();
        assert_eq!(changed, 1);
        assert!(plan.entries.iter().all(|entry| entry.lod_index <= 1));
    }

    #[test]
    fn lod0_budget_keeps_most_significant_subjects() {
        let mut control = HierarchicalLodControlPlane::default();
        let plan = control.evaluate(
            1,
            &[signal(1, 0.99, 3), signal(2, 0.98, 3), signal(3, 0.97, 3)],
            HierarchicalLodConfig {
                lod0_budget: 2,
                ..HierarchicalLodConfig::default()
            },
        );
        assert_eq!(plan.entry(1).unwrap().lod_index, 0);
        assert_eq!(plan.entry(2).unwrap().lod_index, 0);
        assert_eq!(plan.entry(3).unwrap().lod_index, 1);
        assert_eq!(plan.stats.lod0_budget_demotions, 1);
    }

    #[test]
    fn single_lod_subjects_do_not_consume_lod0_budget() {
        let mut control = HierarchicalLodControlPlane::default();
        let plan = control.evaluate(
            1,
            &[signal(1, 0.99, 1), signal(2, 0.80, 1)],
            HierarchicalLodConfig {
                lod0_budget: 0,
                ..HierarchicalLodConfig::default()
            },
        );
        assert_eq!(plan.entry(1).unwrap().lod_index, 0);
        assert_eq!(plan.entry(2).unwrap().lod_index, 0);
        assert_eq!(plan.stats.lod0_requested, 0);
        assert_eq!(plan.stats.lod0_budget_demotions, 0);
    }

    #[test]
    fn forced_lod_bypasses_transition_and_lod0_budget() {
        let mut forced = signal(1, 0.01, 4);
        forced.forced_lod = Some(0);
        let mut control = HierarchicalLodControlPlane::default();
        let plan = control.evaluate(
            1,
            &[forced, signal(2, 0.99, 4)],
            HierarchicalLodConfig {
                lod0_budget: 0,
                transition_budget: 0,
                ..HierarchicalLodConfig::default()
            },
        );
        assert_eq!(plan.entry(1).unwrap().lod_index, 0);
        assert_ne!(plan.entry(2).unwrap().lod_index, 0);
        assert_eq!(plan.stats.forced_lods, 1);
    }

    #[test]
    fn visibility_entry_adapter_preserves_provider_neutral_significance() {
        let visibility = VisibilityPlanEntry {
            subject_id: 99,
            decision: VisibilityDecision::Visible,
            confidence: 0.9,
            result_age_frames: Some(1),
            significance: 0.66,
            streaming_pressure: 700,
            selected_for_query: false,
        };
        let signal = LodSubjectSignal::from_visibility(&visibility, 4);
        assert_eq!(signal.subject_id, 99);
        assert_eq!(signal.authored_lod_count, 4);
        assert!((signal.visibility_significance - 0.66).abs() < 0.001);
    }

    #[test]
    fn update_cadence_degrades_with_significance() {
        assert_eq!(SignificanceTier::Critical.update_interval_frames(), 1);
        assert_eq!(SignificanceTier::Medium.update_interval_frames(), 2);
        assert_eq!(SignificanceTier::Low.update_interval_frames(), 4);
        assert_eq!(SignificanceTier::Background.update_interval_frames(), 8);
    }
}
