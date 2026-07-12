use std::collections::{BTreeMap, BTreeSet};

use newengine_ecs::{EntityId, World};
use newengine_math::{avalanche_u64, Vec3};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto, PhysicsQueryKindDto};
use newengine_sim::CharacterMotor;
use newengine_transform::Transform;

use super::inventory::{
    consume_equipped_ammo, equipped_reserve_ammo, persist_equipped_weapon_state,
    sync_equipped_weapon_runtime, try_collect_item_pickup, EquippedWeaponBinding,
};
use super::{PlayerCommandFrame, PlayerController, PlayerStanceState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitscanWeaponTuning {
    pub magazine_capacity: u32,
    pub reserve_capacity: u32,
    pub fire_interval: f32,
    pub reload_duration: f32,
    pub damage: f32,
    pub range: f32,
    pub hip_spread_radians: f32,
    pub aim_spread_radians: f32,
    pub recoil_pitch_radians: f32,
    pub recoil_yaw_radians: f32,
    pub muzzle_forward_offset: f32,
}

impl Default for HitscanWeaponTuning {
    fn default() -> Self {
        Self {
            magazine_capacity: 30,
            reserve_capacity: 90,
            fire_interval: 0.1,
            reload_duration: 1.8,
            damage: 25.0,
            range: 120.0,
            hip_spread_radians: 1.5_f32.to_radians(),
            aim_spread_radians: 0.25_f32.to_radians(),
            recoil_pitch_radians: 0.8_f32.to_radians(),
            recoil_yaw_radians: 0.35_f32.to_radians(),
            muzzle_forward_offset: 0.52,
        }
    }
}

impl HitscanWeaponTuning {
    pub fn sanitized(self) -> Self {
        Self {
            magazine_capacity: self.magazine_capacity.clamp(1, 10_000),
            reserve_capacity: self.reserve_capacity.min(1_000_000),
            fire_interval: self.fire_interval.clamp(0.01, 60.0),
            reload_duration: self.reload_duration.clamp(0.0, 120.0),
            damage: self.damage.clamp(0.0, 1_000_000.0),
            range: self.range.clamp(0.1, 100_000.0),
            hip_spread_radians: self
                .hip_spread_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            aim_spread_radians: self
                .aim_spread_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            recoil_pitch_radians: self.recoil_pitch_radians.clamp(0.0, 1.0),
            recoil_yaw_radians: self.recoil_yaw_radians.clamp(0.0, 1.0),
            muzzle_forward_offset: self.muzzle_forward_offset.clamp(0.0, 10.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerWeaponState {
    pub ammo_in_magazine: u32,
    pub reserve_ammo: u32,
    pub cooldown_remaining: f32,
    pub reload_remaining: f32,
    pub shot_sequence: u64,
    pub aiming: bool,
    pub empty_latched: bool,
}

impl PlayerWeaponState {
    pub fn loaded(tuning: HitscanWeaponTuning) -> Self {
        let tuning = tuning.sanitized();
        Self {
            ammo_in_magazine: tuning.magazine_capacity,
            reserve_ammo: tuning.reserve_capacity,
            cooldown_remaining: 0.0,
            reload_remaining: 0.0,
            shot_sequence: 0,
            aiming: false,
            empty_latched: false,
        }
    }
}

impl Default for PlayerWeaponState {
    fn default() -> Self {
        Self::loaded(HitscanWeaponTuning::default())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Health {
    pub fn new(maximum: f32) -> Self {
        let maximum = maximum.max(0.0);
        Self {
            current: maximum,
            maximum,
        }
    }

    pub fn apply_damage(&mut self, amount: f32) -> f32 {
        let amount = if amount.is_finite() {
            amount.max(0.0)
        } else {
            0.0
        };
        let before = self.current;
        self.current = (self.current - amount).clamp(0.0, self.maximum.max(0.0));
        before - self.current
    }

    #[inline]
    pub fn alive(self) -> bool {
        self.current > 0.0
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interactable {
    pub prompt: String,
    pub enabled: bool,
}

impl Interactable {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            enabled: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerInteractionTuning {
    pub range: f32,
    pub ray_origin_forward_offset: f32,
}

impl Default for PlayerInteractionTuning {
    fn default() -> Self {
        Self {
            range: 3.0,
            ray_origin_forward_offset: 0.52,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponEventKind {
    Fired,
    Empty,
    ReloadStarted,
    ReloadCompleted,
    Hit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponEvent {
    pub kind: WeaponEventKind,
    pub shooter: EntityId,
    pub target: Option<EntityId>,
    pub shot_sequence: u64,
    pub damage: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Debug, Default)]
pub struct WeaponEventBus {
    pub events: Vec<WeaponEvent>,
}

impl WeaponEventBus {
    fn emit(&mut self, event: WeaponEvent) {
        const CAPACITY: usize = 512;
        if self.events.len() >= CAPACITY {
            let overflow = self.events.len() + 1 - CAPACITY;
            self.events.drain(0..overflow);
        }
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<WeaponEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InteractionEvent {
    pub player: EntityId,
    pub target: EntityId,
    pub prompt: String,
    pub fixed_tick: u64,
    pub point: Vec3,
}

#[derive(Clone, Debug, Default)]
pub struct InteractionEventBus {
    pub events: Vec<InteractionEvent>,
}

impl InteractionEventBus {
    fn emit(&mut self, event: InteractionEvent) {
        const CAPACITY: usize = 256;
        if self.events.len() >= CAPACITY {
            let overflow = self.events.len() + 1 - CAPACITY;
            self.events.drain(0..overflow);
        }
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<InteractionEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PendingHitscan {
    pub query_seq: u64,
    pub shot_sequence: u64,
    pub origin: Vec3,
    pub direction: Vec3,
    pub range: f32,
    pub damage: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PendingInteraction {
    pub query_seq: u64,
    pub origin: Vec3,
    pub direction: Vec3,
    pub range: f32,
}

const HITSCAN_QUERY_SALT: u64 = 0x243f_6a88_85a3_08d3;
const INTERACTION_QUERY_SALT: u64 = 0x1319_8a2e_0370_7344;

#[inline]
fn hitscan_query_seq(player: EntityId, shot_sequence: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ HITSCAN_QUERY_SALT ^ shot_sequence.rotate_left(17))
}

#[inline]
fn interaction_query_seq(player: EntityId, source_frame: u64) -> u64 {
    avalanche_u64(player.stable_u64() ^ INTERACTION_QUERY_SALT ^ source_frame.rotate_left(29))
}

#[inline]
fn signed_unit(seed: u64) -> f32 {
    let value = (avalanche_u64(seed) >> 40) as u32;
    (value as f32 / 0x00ff_ffffu32 as f32) * 2.0 - 1.0
}

fn shot_origin_and_direction(
    world: &World,
    player: EntityId,
    tuning: HitscanWeaponTuning,
    aiming: bool,
    shot_sequence: u64,
) -> Option<(Vec3, Vec3)> {
    let transform = world.get::<Transform>(player).copied()?;
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|stance| stance.current_eye_height)
        .unwrap_or(1.6);
    let forward = (transform.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    let right = (transform.rotation * Vec3::X).normalize_or_zero();
    let up = (transform.rotation * Vec3::Y).normalize_or_zero();
    if forward.length_squared() <= 1.0e-8 {
        return None;
    }

    let spread = if aiming {
        tuning.aim_spread_radians
    } else {
        tuning.hip_spread_radians
    };
    let spread_scale = spread.tan();
    let offset_x = signed_unit(shot_sequence ^ 0x9e37_79b9) * spread_scale;
    let offset_y = signed_unit(shot_sequence ^ 0x7f4a_7c15) * spread_scale;
    let direction = (forward + right * offset_x + up * offset_y).normalize_or_zero();
    let origin = transform.position + Vec3::Y * eye_height + forward * tuning.muzzle_forward_offset;
    Some((origin, direction))
}

fn interaction_ray(
    world: &World,
    player: EntityId,
    tuning: PlayerInteractionTuning,
) -> Option<(Vec3, Vec3)> {
    let transform = world.get::<Transform>(player).copied()?;
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|stance| stance.current_eye_height)
        .unwrap_or(1.6);
    let direction = (transform.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    if direction.length_squared() <= 1.0e-8 {
        return None;
    }
    Some((
        transform.position + Vec3::Y * eye_height + direction * tuning.ray_origin_forward_offset,
        direction,
    ))
}

pub fn step_player_combat(world: &mut World, dt: f32, _fixed_tick: u64) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();

    for player in players {
        // Equipment is authoritative for the active weapon and reserve ammunition.
        // Legacy direct weapon components remain supported when no inventory binding exists.
        sync_equipped_weapon_runtime(world, player);
        let actions = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| commands.actions)
            .unwrap_or_default();
        let source_frame = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| commands.source_frame)
            .unwrap_or(0);
        let tuning = world
            .get::<HitscanWeaponTuning>(player)
            .copied()
            .unwrap_or_default()
            .sanitized();
        if world.get::<PlayerWeaponState>(player).is_none() {
            let _ = world.insert(player, PlayerWeaponState::loaded(tuning));
        }

        let inventory_backed = world.get::<EquippedWeaponBinding>(player).is_some();
        let mut state = world
            .get::<PlayerWeaponState>(player)
            .copied()
            .unwrap_or_else(|| PlayerWeaponState::loaded(tuning));
        if let Some(reserve) = equipped_reserve_ammo(world, player) {
            state.reserve_ammo = reserve;
        }

        let mut events = Vec::<WeaponEvent>::new();
        let mut fire_request = None;
        state.aiming = actions.aim_held;
        state.cooldown_remaining = (state.cooldown_remaining - dt).max(0.0);

        if state.reload_remaining > 0.0 {
            state.reload_remaining = (state.reload_remaining - dt).max(0.0);
            if state.reload_remaining == 0.0 {
                let needed = tuning
                    .magazine_capacity
                    .saturating_sub(state.ammo_in_magazine);
                let moved = if inventory_backed {
                    consume_equipped_ammo(world, player, needed)
                } else {
                    needed.min(state.reserve_ammo)
                };
                state.ammo_in_magazine += moved;
                if inventory_backed {
                    state.reserve_ammo = equipped_reserve_ammo(world, player).unwrap_or(0);
                } else {
                    state.reserve_ammo -= moved;
                }
                events.push(weapon_event(
                    WeaponEventKind::ReloadCompleted,
                    player,
                    state.shot_sequence,
                ));
            }
        }

        if actions.reload_pressed
            && state.reload_remaining <= 0.0
            && state.ammo_in_magazine < tuning.magazine_capacity
            && state.reserve_ammo > 0
        {
            state.reload_remaining = tuning.reload_duration;
            events.push(weapon_event(
                WeaponEventKind::ReloadStarted,
                player,
                state.shot_sequence,
            ));
        }

        if actions.fire_primary_held
            && state.reload_remaining <= 0.0
            && state.cooldown_remaining <= 0.0
        {
            if state.ammo_in_magazine == 0 {
                if !state.empty_latched {
                    events.push(weapon_event(
                        WeaponEventKind::Empty,
                        player,
                        state.shot_sequence,
                    ));
                    state.empty_latched = true;
                }
            } else {
                state.ammo_in_magazine -= 1;
                state.shot_sequence = state.shot_sequence.wrapping_add(1);
                state.cooldown_remaining = tuning.fire_interval;
                state.empty_latched = false;
                fire_request = Some((state.shot_sequence, state.aiming));
                events.push(weapon_event(
                    WeaponEventKind::Fired,
                    player,
                    state.shot_sequence,
                ));
            }
        } else if !actions.fire_primary_held {
            state.empty_latched = false;
        }

        let _ = world.insert(player, state);
        persist_equipped_weapon_state(world, player);

        if let Some((shot_sequence, aiming)) = fire_request {
            if let Some((origin, direction)) =
                shot_origin_and_direction(world, player, tuning, aiming, shot_sequence)
            {
                let pending = PendingHitscan {
                    query_seq: hitscan_query_seq(player, shot_sequence),
                    shot_sequence,
                    origin,
                    direction,
                    range: tuning.range,
                    damage: tuning.damage,
                };
                let _ = world.insert(player, pending);
                apply_recoil(world, player, tuning, shot_sequence);
            }
        }

        if actions.interact_pressed {
            let interaction_tuning = world
                .get::<PlayerInteractionTuning>(player)
                .copied()
                .unwrap_or_default();
            if let Some((origin, direction)) = interaction_ray(world, player, interaction_tuning) {
                let _ = world.insert(
                    player,
                    PendingInteraction {
                        query_seq: interaction_query_seq(player, source_frame),
                        origin,
                        direction,
                        range: interaction_tuning.range.clamp(0.1, 100.0),
                    },
                );
            }
        }

        for event in events {
            emit_weapon_event(world, event);
        }
    }
}

fn apply_recoil(
    world: &mut World,
    player: EntityId,
    tuning: HitscanWeaponTuning,
    shot_sequence: u64,
) {
    let Some(motor) = world.get_mut::<CharacterMotor>(player) else {
        return;
    };
    let yaw_sign = if shot_sequence.is_multiple_of(2) {
        1.0
    } else {
        -1.0
    };
    let yaw_scale = 0.55 + signed_unit(shot_sequence ^ 0xa409_3822).abs() * 0.45;
    motor.pitch =
        (motor.pitch - tuning.recoil_pitch_radians).clamp(-motor.pitch_limit, motor.pitch_limit);
    motor.yaw += tuning.recoil_yaw_radians * yaw_sign * yaw_scale;
}

fn weapon_event(kind: WeaponEventKind, shooter: EntityId, shot_sequence: u64) -> WeaponEvent {
    WeaponEvent {
        kind,
        shooter,
        target: None,
        shot_sequence,
        damage: 0.0,
        point: Vec3::ZERO,
        normal: Vec3::ZERO,
    }
}

fn emit_weapon_event(world: &mut World, event: WeaponEvent) {
    if world.resource::<WeaponEventBus>().is_none() {
        world.insert_resource(WeaponEventBus::default());
    }
    if let Some(bus) = world.resource_mut::<WeaponEventBus>() {
        bus.emit(event);
    }
}

fn emit_interaction_event(world: &mut World, event: InteractionEvent) {
    if world.resource::<InteractionEventBus>().is_none() {
        world.insert_resource(InteractionEventBus::default());
    }
    if let Some(bus) = world.resource_mut::<InteractionEventBus>() {
        bus.emit(event);
    }
}

#[inline]
fn vec3_to_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

#[inline]
fn vec3_from_array(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

pub(crate) fn collect_combat_queries(world: &World) -> Vec<PhysicsQueryDto> {
    let mut queries = Vec::new();
    for (_, pending) in world.query::<PendingHitscan>() {
        queries.push(PhysicsQueryDto {
            seq: pending.query_seq,
            kind: PhysicsQueryKindDto::Ray {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.range,
            },
        });
    }
    for (_, pending) in world.query::<PendingInteraction>() {
        queries.push(PhysicsQueryDto {
            seq: pending.query_seq,
            kind: PhysicsQueryKindDto::Ray {
                origin: vec3_to_array(pending.origin),
                dir: vec3_to_array(pending.direction),
                max_t: pending.range,
            },
        });
    }
    queries
}

/// Resolves all pending hitscan/interaction requests and returns query sequences consumed by this
/// subsystem. Pending requests are always removed, including misses, so a render or fixed tick can
/// never replay the same shot/interaction.
pub(crate) fn resolve_combat_queries(
    world: &mut World,
    fixed_tick: u64,
    hits: &[PhysicsQueryHitDto],
    key_to_entity: &BTreeMap<u64, EntityId>,
) -> BTreeSet<u64> {
    let mut consumed = BTreeSet::new();
    let pending_shots = world
        .query::<PendingHitscan>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (shooter, pending) in pending_shots {
        consumed.insert(pending.query_seq);
        if let Some(hit) = hits.iter().find(|hit| hit.seq == pending.query_seq) {
            let target = key_to_entity.get(&hit.entity).copied();
            let applied_damage = target
                .and_then(|target| world.get_mut::<Health>(target))
                .map(|health| health.apply_damage(pending.damage))
                .unwrap_or(0.0);
            emit_weapon_event(
                world,
                WeaponEvent {
                    kind: WeaponEventKind::Hit,
                    shooter,
                    target,
                    shot_sequence: pending.shot_sequence,
                    damage: applied_damage,
                    point: vec3_from_array(hit.position),
                    normal: vec3_from_array(hit.normal),
                },
            );
        }
        let _ = world.remove::<PendingHitscan>(shooter);
    }

    let pending_interactions = world
        .query::<PendingInteraction>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (player, pending) in pending_interactions {
        consumed.insert(pending.query_seq);
        if let Some(hit) = hits.iter().find(|hit| hit.seq == pending.query_seq) {
            if let Some(target) = key_to_entity.get(&hit.entity).copied() {
                if let Some(interactable) = world.get::<Interactable>(target).cloned() {
                    if interactable.enabled {
                        emit_interaction_event(
                            world,
                            InteractionEvent {
                                player,
                                target,
                                prompt: interactable.prompt,
                                fixed_tick,
                                point: vec3_from_array(hit.position),
                            },
                        );
                        let _ = try_collect_item_pickup(world, player, target);
                    }
                }
            }
        }
        let _ = world.remove::<PendingInteraction>(player);
    }
    consumed
}

pub fn drain_weapon_events(world: &mut World) -> Vec<WeaponEvent> {
    world
        .resource_mut::<WeaponEventBus>()
        .map(WeaponEventBus::drain)
        .unwrap_or_default()
}

pub fn drain_interaction_events(world: &mut World) -> Vec<InteractionEvent> {
    world
        .resource_mut::<InteractionEventBus>()
        .map(InteractionEventBus::drain)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::{
        default_rifle_ammo_id, inventory_quantity, remove_item, spawn_default_player,
        PlayerStanceState,
    };
    use newengine_physics_api::PhysicsQueryHitDto;

    #[test]
    fn weapon_fires_reloads_and_applies_typed_damage() {
        let mut world = World::new();
        let shooter = spawn_default_player(&mut world, None, "shooter", Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(target, Health::new(100.0));
        let _ = world.insert(target, Transform::default());
        let _ = world.insert(shooter, HitscanWeaponTuning::default());
        let _ = world.insert(shooter, PlayerWeaponState::default());
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(shooter) {
            commands.actions.fire_primary_held = true;
        }

        step_player_combat(&mut world, 1.0 / 60.0, 1);
        let pending = world
            .get::<PendingHitscan>(shooter)
            .copied()
            .expect("pending hitscan");
        let map = BTreeMap::from([
            (shooter.stable_u64(), shooter),
            (target.stable_u64(), target),
        ]);
        resolve_combat_queries(
            &mut world,
            1,
            &[PhysicsQueryHitDto {
                seq: pending.query_seq,
                entity: target.stable_u64(),
                position: [0.0, 0.0, -2.0],
                normal: [0.0, 0.0, 1.0],
                distance: 2.0,
            }],
            &map,
        );

        assert_eq!(world.get::<Health>(target).expect("health").current, 75.0);
        let events = drain_weapon_events(&mut world);
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::Fired));
        assert!(events.iter().any(|event| {
            event.kind == WeaponEventKind::Hit
                && event.target == Some(target)
                && event.damage == 25.0
        }));
    }

    #[test]
    fn reload_state_machine_transfers_ammo_after_fixed_duration() {
        let mut world = World::new();
        let player =
            crate::gameplay::spawn_default_player(&mut world, None, "reload-player", Vec3::ZERO);
        let tuning = HitscanWeaponTuning {
            reload_duration: 0.02,
            ..HitscanWeaponTuning::default()
        };
        let _ = world.insert(player, tuning);
        let ammo_item = default_rifle_ammo_id();
        let reserve_before = inventory_quantity(&world, player, ammo_item);
        remove_item(
            &mut world,
            player,
            ammo_item,
            reserve_before.saturating_sub(10),
        )
        .expect("trim inventory ammunition");
        let _ = world.insert(
            player,
            PlayerWeaponState {
                ammo_in_magazine: 0,
                reserve_ammo: 10,
                ..PlayerWeaponState::loaded(tuning)
            },
        );
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.reload_pressed = true;
        }

        step_player_combat(&mut world, 0.01, 1);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.reload_pressed = false;
        }
        step_player_combat(&mut world, 0.02, 2);

        let state = world
            .get::<PlayerWeaponState>(player)
            .expect("weapon state");
        assert_eq!(state.ammo_in_magazine, 10);
        assert_eq!(state.reserve_ammo, 0);
        assert_eq!(inventory_quantity(&world, player, ammo_item), 0);
        assert_eq!(state.reload_remaining, 0.0);
        let events = drain_weapon_events(&mut world);
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::ReloadStarted));
        assert!(events
            .iter()
            .any(|event| event.kind == WeaponEventKind::ReloadCompleted));
    }

    #[test]
    fn interaction_query_emits_typed_target_event() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "player", Vec3::ZERO);
        let target = world.spawn();
        let _ = world.insert(target, Interactable::new("Open terminal"));
        let _ = world.insert(target, Transform::default());
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 7;
            commands.actions.interact_pressed = true;
        }
        // Ensure the test uses a stable first-person eye origin.
        let _ = world.insert(player, PlayerStanceState::standing(0.72));

        step_player_combat(&mut world, 1.0 / 60.0, 2);
        let pending = world
            .get::<PendingInteraction>(player)
            .copied()
            .expect("pending interaction");
        let map = BTreeMap::from([(player.stable_u64(), player), (target.stable_u64(), target)]);
        resolve_combat_queries(
            &mut world,
            2,
            &[PhysicsQueryHitDto {
                seq: pending.query_seq,
                entity: target.stable_u64(),
                position: [0.0, 0.7, -1.0],
                normal: [0.0, 0.0, 1.0],
                distance: 1.0,
            }],
            &map,
        );

        let events = drain_interaction_events(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].player, player);
        assert_eq!(events[0].target, target);
        assert_eq!(events[0].prompt, "Open terminal");
    }
}
