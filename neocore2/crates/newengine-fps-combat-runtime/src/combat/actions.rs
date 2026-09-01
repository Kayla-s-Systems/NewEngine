use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ReloadActionStep {
    pub magazine_detached: bool,
    pub ammo_committed: bool,
    pub magazine_inserted: bool,
    pub chambered: bool,
    pub completed: bool,
}

impl ReloadActionStep {
    fn mark(&mut self, phase: WeaponReloadPhase) {
        match phase {
            WeaponReloadPhase::MagazineDetached => self.magazine_detached = true,
            WeaponReloadPhase::AmmoCommitted => self.ammo_committed = true,
            WeaponReloadPhase::MagazineInserted => self.magazine_inserted = true,
            WeaponReloadPhase::Chambered => self.chambered = true,
            WeaponReloadPhase::Complete => self.completed = true,
            WeaponReloadPhase::None | WeaponReloadPhase::Started => {}
        }
    }

    fn merge(&mut self, other: Self) {
        self.magazine_detached |= other.magazine_detached;
        self.ammo_committed |= other.ammo_committed;
        self.magazine_inserted |= other.magazine_inserted;
        self.chambered |= other.chambered;
        self.completed |= other.completed;
    }
}

pub(super) fn ensure_weapon_action_runtime(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
) {
    let current = world.get::<WeaponActionRuntime>(player).copied();
    if current.is_none_or(|state| state.weapon_instance_id != weapon_instance_id) {
        let _ = world.insert(player, WeaponActionRuntime::ready(weapon_instance_id));
        let _ = drain_weapon_reload_animation_markers(world, player, weapon_instance_id);
    }
}

pub(super) fn firing_action(pattern: FiringPatternDefinition) -> (WeaponActionKind, f32) {
    let pattern = pattern.sanitized();
    match pattern.kind {
        FiringPatternKind::Pump | FiringPatternKind::BoltAction => (
            WeaponActionKind::Cycling,
            pattern
                .burst_cooldown
                .max(pattern.time_between_bursts)
                .max(pattern.time_between_shots),
        ),
        _ => (WeaponActionKind::Firing, pattern.time_between_shots),
    }
}

pub(super) fn mark_transient_weapon_action(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    action: WeaponActionKind,
    duration_seconds: f32,
) {
    let duration_seconds = if duration_seconds.is_finite() {
        duration_seconds.max(0.0)
    } else {
        0.0
    };
    let _ = world.insert(
        player,
        WeaponActionRuntime {
            weapon_instance_id,
            action,
            reload_phase: WeaponReloadPhase::None,
            timing_source: WeaponActionTimingSource::TimelineFallback,
            elapsed_seconds: 0.0,
            duration_seconds,
            phase_mask: 0,
        },
    );
}

pub(super) fn step_transient_weapon_action(world: &mut World, player: EntityId, dt: f32) {
    let Some(mut state) = world.get::<WeaponActionRuntime>(player).copied() else {
        return;
    };
    if matches!(
        state.action,
        WeaponActionKind::Ready | WeaponActionKind::Reloading
    ) {
        return;
    }
    state.elapsed_seconds = (state.elapsed_seconds + dt.max(0.0)).min(60.0);
    if state.elapsed_seconds >= state.duration_seconds {
        state = WeaponActionRuntime::ready(state.weapon_instance_id);
    }
    let _ = world.insert(player, state);
}

pub(super) fn reload_timing_source(
    world: &World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    fallback_duration_seconds: f32,
) -> (WeaponActionTimingSource, f32) {
    let authority = world
        .get::<WeaponReloadAnimationAuthority>(player)
        .copied()
        .filter(|authority| authority.weapon_instance_id == weapon_instance_id)
        .filter(|authority| authority.is_complete());
    match authority {
        Some(authority) => (
            WeaponActionTimingSource::AnimationMarkers,
            authority.clip_duration_seconds,
        ),
        None => (
            WeaponActionTimingSource::TimelineFallback,
            fallback_duration_seconds,
        ),
    }
}

pub(super) fn begin_reload_action(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    duration_seconds: f32,
    timing_source: WeaponActionTimingSource,
) {
    let _ = drain_weapon_reload_animation_markers(world, player, weapon_instance_id);
    let _ = world.insert(
        player,
        WeaponActionRuntime::begin_reload(weapon_instance_id, duration_seconds, timing_source),
    );
}

fn apply_reload_phase(
    state: &mut WeaponActionRuntime,
    phase: WeaponReloadPhase,
) -> ReloadActionStep {
    let bit = phase.marker_bit();
    if bit == 0 || state.phase_mask & bit != 0 {
        return ReloadActionStep::default();
    }
    state.phase_mask |= bit;
    state.reload_phase = phase;
    if phase == WeaponReloadPhase::Complete {
        state.action = WeaponActionKind::Ready;
        state.elapsed_seconds = state.duration_seconds.max(state.elapsed_seconds);
    }
    let mut step = ReloadActionStep::default();
    step.mark(phase);
    step
}

pub(super) fn apply_reload_animation_markers(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
) -> ReloadActionStep {
    let markers = drain_weapon_reload_animation_markers(world, player, weapon_instance_id);
    if markers.is_empty() {
        return ReloadActionStep::default();
    }
    let Some(mut state) = world
        .get::<WeaponActionRuntime>(player)
        .copied()
        .filter(|state| {
            state.weapon_instance_id == weapon_instance_id
                && state.action == WeaponActionKind::Reloading
                && state.timing_source == WeaponActionTimingSource::AnimationMarkers
        })
    else {
        return ReloadActionStep::default();
    };
    let mut step = ReloadActionStep::default();
    for marker in markers {
        step.merge(apply_reload_phase(&mut state, marker.phase));
    }
    let _ = world.insert(player, state);
    step
}

pub(super) fn step_reload_action(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    timeline: WeaponReloadTimelineProfile,
    dt: f32,
) -> ReloadActionStep {
    let timeline = timeline.sanitized();
    let mut step = apply_reload_animation_markers(world, player, weapon_instance_id);
    let Some(mut state) = world
        .get::<WeaponActionRuntime>(player)
        .copied()
        .filter(|state| {
            state.weapon_instance_id == weapon_instance_id
                && state.action == WeaponActionKind::Reloading
        })
    else {
        return step;
    };
    state.elapsed_seconds = (state.elapsed_seconds + dt.max(0.0)).min(120.0);
    if state.timing_source == WeaponActionTimingSource::AnimationMarkers {
        let _ = world.insert(player, state);
        return step;
    }

    let progress = state.progress();
    for (threshold, phase) in [
        (
            timeline.magazine_detach_fraction,
            WeaponReloadPhase::MagazineDetached,
        ),
        (
            timeline.ammo_commit_fraction,
            WeaponReloadPhase::AmmoCommitted,
        ),
        (
            timeline.magazine_insert_fraction,
            WeaponReloadPhase::MagazineInserted,
        ),
        (timeline.chamber_fraction, WeaponReloadPhase::Chambered),
        (1.0, WeaponReloadPhase::Complete),
    ] {
        if progress >= threshold {
            step.merge(apply_reload_phase(&mut state, phase));
        }
    }
    let _ = world.insert(player, state);
    step
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::queue_weapon_reload_animation_marker;

    #[test]
    fn firing_patterns_resolve_to_fire_or_cycle_actions() {
        let semi = FiringPatternDefinition {
            kind: FiringPatternKind::Semi,
            time_between_shots: 0.12,
            ..FiringPatternDefinition::default()
        };
        let pump = FiringPatternDefinition {
            kind: FiringPatternKind::Pump,
            time_between_shots: 0.2,
            time_between_bursts: 0.45,
            burst_cooldown: 0.6,
            ..FiringPatternDefinition::default()
        };
        let bolt = FiringPatternDefinition {
            kind: FiringPatternKind::BoltAction,
            time_between_shots: 0.2,
            time_between_bursts: 0.55,
            burst_cooldown: 0.4,
            ..FiringPatternDefinition::default()
        };

        assert_eq!(firing_action(semi), (WeaponActionKind::Firing, 0.12));
        assert_eq!(firing_action(pump), (WeaponActionKind::Cycling, 0.6));
        assert_eq!(firing_action(bolt), (WeaponActionKind::Cycling, 0.55));
    }

    #[test]
    fn fallback_reload_timeline_markers_fire_once_when_frame_crosses_multiple_thresholds() {
        let mut world = World::new();
        let player = world.spawn();
        let instance = ItemInstanceId(41);
        begin_reload_action(
            &mut world,
            player,
            instance,
            2.0,
            WeaponActionTimingSource::TimelineFallback,
        );
        let timeline = WeaponReloadTimelineProfile::default();

        let first = step_reload_action(&mut world, player, instance, timeline, 1.5);
        assert!(first.magazine_detached);
        assert!(first.ammo_committed);
        assert!(first.magazine_inserted);
        assert!(!first.chambered);
        assert!(!first.completed);

        let second = step_reload_action(&mut world, player, instance, timeline, 0.5);
        assert!(!second.magazine_detached);
        assert!(!second.ammo_committed);
        assert!(!second.magazine_inserted);
        assert!(second.chambered);
        assert!(second.completed);
    }

    #[test]
    fn animation_authority_disables_percentage_phase_commits() {
        let mut world = World::new();
        let player = world.spawn();
        let instance = ItemInstanceId(51);
        begin_reload_action(
            &mut world,
            player,
            instance,
            1.0,
            WeaponActionTimingSource::AnimationMarkers,
        );

        let timeline_step = step_reload_action(
            &mut world,
            player,
            instance,
            WeaponReloadTimelineProfile::default(),
            1.0,
        );
        assert_eq!(timeline_step, ReloadActionStep::default());
        assert_eq!(
            world.get::<WeaponActionRuntime>(player).unwrap().action,
            WeaponActionKind::Reloading,
            "elapsed action duration cannot complete an animation-authoritative reload"
        );

        queue_weapon_reload_animation_marker(
            &mut world,
            player,
            WeaponReloadAnimationMarker {
                weapon_instance_id: instance,
                phase: WeaponReloadPhase::AmmoCommitted,
                clip_time_seconds: 0.6,
                playback_time_seconds: 0.6,
                loop_index: 0,
            },
        );
        let marker_step = apply_reload_animation_markers(&mut world, player, instance);
        assert!(marker_step.ammo_committed);
        assert!(!marker_step.completed);

        queue_weapon_reload_animation_marker(
            &mut world,
            player,
            WeaponReloadAnimationMarker {
                weapon_instance_id: instance,
                phase: WeaponReloadPhase::Complete,
                clip_time_seconds: 1.0,
                playback_time_seconds: 1.0,
                loop_index: 0,
            },
        );
        let complete = apply_reload_animation_markers(&mut world, player, instance);
        assert!(complete.completed);
        assert_eq!(
            world.get::<WeaponActionRuntime>(player).unwrap().action,
            WeaponActionKind::Ready
        );
    }
}
