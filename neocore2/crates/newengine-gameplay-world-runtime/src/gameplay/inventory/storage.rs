use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InventoryEntry {
    pub instance_id: ItemInstanceId,
    pub item: ItemId,
    pub quantity: u32,
    pub condition: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InventoryCapacityState {
    pub used_slots: u32,
    pub slot_capacity: u32,
    pub total_weight: f32,
    pub weight_capacity: f32,
}

impl InventoryCapacityState {
    #[inline]
    pub fn free_slots(self) -> u32 {
        self.slot_capacity.saturating_sub(self.used_slots)
    }

    #[inline]
    pub fn free_weight(self) -> f32 {
        (self.weight_capacity - self.total_weight).max(0.0)
    }

    #[inline]
    pub fn slot_fill(self) -> f32 {
        if self.slot_capacity == 0 {
            1.0
        } else {
            (self.used_slots as f32 / self.slot_capacity as f32).clamp(0.0, 1.0)
        }
    }

    #[inline]
    pub fn weight_fill(self) -> f32 {
        if self.weight_capacity <= f32::EPSILON {
            (self.total_weight > f32::EPSILON) as u8 as f32
        } else {
            (self.total_weight / self.weight_capacity).clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInventory {
    pub slot_capacity: u32,
    pub weight_capacity: f32,
    pub entries: Vec<InventoryEntry>,
    pub equipped: BTreeMap<EquipmentSlot, ItemInstanceId>,
    pub active_slot: Option<EquipmentSlot>,
    pub weapon_states: BTreeMap<ItemInstanceId, PlayerWeaponState>,
    pub weapon_components: BTreeMap<ItemInstanceId, BTreeMap<String, WeaponComponentInstance>>,
    loadout_initialized: bool,
    pub(super) next_instance_serial: u64,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self {
            slot_capacity: 24,
            weight_capacity: 80.0,
            entries: Vec::new(),
            equipped: BTreeMap::new(),
            active_slot: None,
            weapon_states: BTreeMap::new(),
            weapon_components: BTreeMap::new(),
            loadout_initialized: false,
            next_instance_serial: 1,
        }
    }
}

impl PlayerInventory {
    #[inline]
    pub fn quantity(&self, item: ItemId) -> u32 {
        self.entries
            .iter()
            .filter(|entry| entry.item == item)
            .fold(0u32, |total, entry| total.saturating_add(entry.quantity))
    }

    #[inline]
    pub const fn loadout_initialized(&self) -> bool {
        self.loadout_initialized
    }

    #[inline]
    pub fn mark_loadout_initialized(&mut self) {
        self.loadout_initialized = true;
    }

    #[inline]
    pub fn used_slots(&self) -> u32 {
        self.entries.len().min(u32::MAX as usize) as u32
    }

    pub fn total_weight(&self, catalog: &ItemCatalog) -> f32 {
        self.entries.iter().fold(0.0, |total, entry| {
            let weight = catalog
                .get(entry.item)
                .map(|definition| definition.unit_weight)
                .unwrap_or(0.0);
            total + weight * entry.quantity as f32
        })
    }

    #[inline]
    pub fn capacity_state(&self, catalog: &ItemCatalog) -> InventoryCapacityState {
        InventoryCapacityState {
            used_slots: self.used_slots(),
            slot_capacity: self.slot_capacity,
            total_weight: self.total_weight(catalog),
            weight_capacity: self.weight_capacity,
        }
    }

    pub(super) fn move_instance_to_index(
        &mut self,
        instance: ItemInstanceId,
        target_index: usize,
    ) -> Result<bool, String> {
        let source_index = self
            .entries
            .iter()
            .position(|entry| entry.instance_id == instance)
            .ok_or_else(|| "inventory instance is not present".to_owned())?;
        if self.entries.len() <= 1 {
            return Ok(false);
        }
        let mut insertion = target_index.min(self.entries.len() - 1);
        if source_index == insertion {
            return Ok(false);
        }
        let entry = self.entries.remove(source_index);
        if source_index < insertion {
            insertion = insertion.saturating_sub(1);
        }
        insertion = insertion.min(self.entries.len());
        self.entries.insert(insertion, entry);
        Ok(true)
    }

    pub(super) fn split_stack(
        &mut self,
        owner: EntityId,
        instance: ItemInstanceId,
        quantity: u32,
        catalog: &ItemCatalog,
    ) -> Result<ItemInstanceId, String> {
        if quantity == 0 {
            return Err("split quantity must be greater than zero".to_owned());
        }
        if self.used_slots() >= self.slot_capacity {
            return Err("inventory has no free slot for split stack".to_owned());
        }
        let source_index = self
            .entries
            .iter()
            .position(|entry| entry.instance_id == instance)
            .ok_or_else(|| "inventory instance is not present".to_owned())?;
        let source = self.entries[source_index];
        let definition = catalog
            .get(source.item)
            .ok_or_else(|| "item definition is unavailable".to_owned())?;
        if definition.max_stack <= 1 {
            return Err("item is not stackable".to_owned());
        }
        if quantity >= source.quantity {
            return Err("split quantity must be smaller than source stack".to_owned());
        }
        let new_instance = self.allocate_instance(owner, source.item);
        self.entries[source_index].quantity -= quantity;
        self.entries.insert(
            source_index + 1,
            InventoryEntry {
                instance_id: new_instance,
                item: source.item,
                quantity,
                condition: source.condition,
            },
        );
        Ok(new_instance)
    }

    pub(super) fn merge_stack_instances(
        &mut self,
        source: ItemInstanceId,
        target: ItemInstanceId,
        catalog: &ItemCatalog,
    ) -> Result<InventoryMutation, String> {
        if source == target {
            return Ok(InventoryMutation::default());
        }
        let source_index = self
            .entries
            .iter()
            .position(|entry| entry.instance_id == source)
            .ok_or_else(|| "source inventory instance is not present".to_owned())?;
        let target_index = self
            .entries
            .iter()
            .position(|entry| entry.instance_id == target)
            .ok_or_else(|| "target inventory instance is not present".to_owned())?;
        let source_entry = self.entries[source_index];
        let target_entry = self.entries[target_index];
        if source_entry.item != target_entry.item {
            return Err("only identical item definitions can be merged".to_owned());
        }
        if (source_entry.condition - target_entry.condition).abs() > 1.0e-4 {
            return Err("stacks with different condition cannot be merged".to_owned());
        }
        let definition = catalog
            .get(source_entry.item)
            .ok_or_else(|| "item definition is unavailable".to_owned())?;
        let max_stack = definition.max_stack.max(1);
        if max_stack <= 1 {
            return Err("item is not stackable".to_owned());
        }
        let available = max_stack.saturating_sub(target_entry.quantity);
        let moved = source_entry.quantity.min(available);
        if moved == 0 {
            return Ok(InventoryMutation {
                accepted: 0,
                rejected: source_entry.quantity,
                touched_instances: vec![source, target],
            });
        }
        self.entries[target_index].quantity += moved;
        self.entries[source_index].quantity -= moved;
        if self.entries[source_index].quantity == 0 {
            let removed = self.entries.remove(source_index);
            self.weapon_states.remove(&removed.instance_id);
            self.weapon_components.remove(&removed.instance_id);
            self.equipped
                .retain(|_, equipped_instance| *equipped_instance != removed.instance_id);
            if self
                .active_slot
                .is_some_and(|slot| !self.equipped.contains_key(&slot))
            {
                self.active_slot = None;
            }
        }
        Ok(InventoryMutation {
            accepted: moved,
            rejected: source_entry.quantity.saturating_sub(moved),
            touched_instances: vec![source, target],
        })
    }

    #[inline]
    pub fn entry(&self, instance: ItemInstanceId) -> Option<&InventoryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.instance_id == instance)
    }

    #[inline]
    pub fn equipped_instance(&self, slot: EquipmentSlot) -> Option<ItemInstanceId> {
        self.equipped.get(&slot).copied()
    }

    fn allocate_instance(&mut self, owner: EntityId, item: ItemId) -> ItemInstanceId {
        loop {
            let serial = self.next_instance_serial;
            self.next_instance_serial = self.next_instance_serial.wrapping_add(1).max(1);
            let candidate = ItemInstanceId(mix64(
                owner.stable_u64()
                    ^ item.0.rotate_left(19)
                    ^ serial.wrapping_mul(0x9e37_79b9_7f4a_7c15),
            ));
            if candidate.0 != 0 && self.entry(candidate).is_none() {
                return candidate;
            }
        }
    }

    pub(super) fn add_definition(
        &mut self,
        owner: EntityId,
        definition: &ItemDefinition,
        requested: u32,
        catalog: &ItemCatalog,
    ) -> InventoryMutation {
        if requested == 0 {
            return InventoryMutation::default();
        }

        self.slot_capacity = self.slot_capacity.clamp(1, 100_000);
        self.weight_capacity = sanitize_non_negative(self.weight_capacity);
        let max_stack = definition.max_stack.max(1);
        let mut remaining = requested;
        let mut touched = Vec::new();

        let weight_allowance = if definition.unit_weight <= f32::EPSILON {
            u32::MAX
        } else {
            let free_weight = (self.weight_capacity - self.total_weight(catalog)).max(0.0);
            (free_weight / definition.unit_weight)
                .floor()
                .clamp(0.0, u32::MAX as f32) as u32
        };
        remaining = remaining.min(weight_allowance);
        let weight_rejected = requested.saturating_sub(remaining);

        for entry in self
            .entries
            .iter_mut()
            .filter(|entry| entry.item == definition.id && entry.quantity < max_stack)
        {
            if remaining == 0 {
                break;
            }
            let moved = remaining.min(max_stack - entry.quantity);
            entry.quantity += moved;
            remaining -= moved;
            touched.push(entry.instance_id);
        }

        while remaining > 0 && self.used_slots() < self.slot_capacity {
            let quantity = remaining.min(max_stack);
            let instance_id = self.allocate_instance(owner, definition.id);
            self.entries.push(InventoryEntry {
                instance_id,
                item: definition.id,
                quantity,
                condition: 1.0,
            });
            if definition.kind == ItemKind::Weapon
                && !definition.weapon_components.default_installed.is_empty()
            {
                let installed = definition
                    .weapon_components
                    .default_installed
                    .iter()
                    .map(|(slot, component_id)| {
                        (
                            slot.clone(),
                            WeaponComponentInstance {
                                component_id: component_id.clone(),
                                active: true,
                            },
                        )
                    })
                    .collect();
                self.weapon_components.insert(instance_id, installed);
            }
            touched.push(instance_id);
            remaining -= quantity;
        }

        let accepted_by_weight = requested.saturating_sub(weight_rejected);
        let accepted = accepted_by_weight.saturating_sub(remaining);
        InventoryMutation {
            accepted,
            rejected: requested.saturating_sub(accepted),
            touched_instances: touched,
        }
    }

    pub(super) fn remove_quantity(&mut self, item: ItemId, requested: u32) -> InventoryMutation {
        if requested == 0 {
            return InventoryMutation::default();
        }
        let mut remaining = requested;
        let mut touched = Vec::new();
        let mut index = self.entries.len();
        while index > 0 && remaining > 0 {
            index -= 1;
            if self.entries[index].item != item {
                continue;
            }
            let moved = remaining.min(self.entries[index].quantity);
            self.entries[index].quantity -= moved;
            remaining -= moved;
            touched.push(self.entries[index].instance_id);
            if self.entries[index].quantity == 0 {
                let removed = self.entries.remove(index);
                self.weapon_states.remove(&removed.instance_id);
                self.weapon_components.remove(&removed.instance_id);
                self.equipped
                    .retain(|_, instance| *instance != removed.instance_id);
                if self
                    .active_slot
                    .is_some_and(|slot| !self.equipped.contains_key(&slot))
                {
                    self.active_slot = None;
                }
            }
        }
        InventoryMutation {
            accepted: requested - remaining,
            rejected: remaining,
            touched_instances: touched,
        }
    }

    pub(super) fn remove_instance_quantity(
        &mut self,
        instance: ItemInstanceId,
        requested: u32,
    ) -> InventoryMutation {
        if requested == 0 {
            return InventoryMutation::default();
        }
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.instance_id == instance)
        else {
            return InventoryMutation {
                accepted: 0,
                rejected: requested,
                touched_instances: Vec::new(),
            };
        };
        let moved = requested.min(self.entries[index].quantity);
        self.entries[index].quantity -= moved;
        if self.entries[index].quantity == 0 {
            let removed = self.entries.remove(index);
            self.weapon_states.remove(&removed.instance_id);
            self.weapon_components.remove(&removed.instance_id);
            self.equipped
                .retain(|_, equipped_instance| *equipped_instance != removed.instance_id);
            if self
                .active_slot
                .is_some_and(|slot| !self.equipped.contains_key(&slot))
            {
                self.active_slot = None;
            }
        }
        InventoryMutation {
            accepted: moved,
            rejected: requested.saturating_sub(moved),
            touched_instances: (moved > 0).then_some(instance).into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InventoryMutation {
    pub accepted: u32,
    pub rejected: u32,
    pub touched_instances: Vec<ItemInstanceId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquippedWeaponBinding {
    pub instance_id: ItemInstanceId,
    pub item: ItemId,
    /// Inventory slot for authored weapons. Virtual Unarmed has no inventory slot.
    pub slot: Option<EquipmentSlot>,
    pub weapon: WeaponItemDefinition,
}

impl EquippedWeaponBinding {
    #[inline]
    pub const fn class(self) -> WeaponType {
        self.weapon.weapon_type
    }

    #[inline]
    pub const fn rank(self) -> u16 {
        self.weapon.rank
    }

    #[inline]
    pub const fn capabilities(self) -> WeaponCapabilities {
        self.weapon.capabilities()
    }

    #[inline]
    pub const fn is_unarmed(self) -> bool {
        self.instance_id.is_unarmed()
    }
}

/// Direct owner -> equipped weapon ECS entity link. Inventory owns selection; the weapon itself
/// owns skeleton pose, sockets, effects and presentation state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EquippedWeaponEntity {
    pub entity: EntityId,
    pub instance_id: ItemInstanceId,
    pub item: ItemId,
}

/// Identity carried by the weapon root entity itself. This keeps downstream systems data-oriented:
/// they consume a normal ECS entity rather than reconstructing a special weapon object from player
/// state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponEntityRuntime {
    pub owner: EntityId,
    pub instance_id: ItemInstanceId,
    pub item: ItemId,
}

/// World-space pose and measured motion of an authored socket on a weapon entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponSocketPose {
    pub position: Vec3,
    pub rotation: newengine_math::Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
}

impl WeaponSocketPose {
    #[inline]
    pub fn stationary(position: Vec3, rotation: newengine_math::Quat) -> Option<Self> {
        let rotation = rotation.normalize_or_identity();
        (position.is_finite() && rotation.is_finite()).then_some(Self {
            position,
            rotation,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
        })
    }

    /// Reconstructs socket linear/angular velocity from consecutive authored skeleton frames.
    /// The algorithm is generic and has no knowledge of muzzle/ejection semantics.
    pub fn with_measured_motion(self, previous: Option<Self>, dt: f32) -> Self {
        let Some(previous) = previous.filter(|_| dt.is_finite() && dt > 1.0e-6 && dt <= 0.25)
        else {
            return self;
        };
        let linear_velocity = (self.position - previous.position) / dt;
        let mut delta = (previous.rotation.inverse() * self.rotation).normalize_or_identity();
        if delta.w < 0.0 {
            delta = newengine_math::Quat::from_xyzw(-delta.x, -delta.y, -delta.z, -delta.w);
        }
        let w = delta.w.clamp(-1.0, 1.0);
        let sin_half = (1.0 - w * w).max(0.0).sqrt();
        let angular_velocity = if sin_half <= 1.0e-6 {
            Vec3::new(delta.x, delta.y, delta.z) * (2.0 / dt)
        } else {
            let angle = 2.0 * sin_half.atan2(w);
            Vec3::new(delta.x, delta.y, delta.z) * (angle / (sin_half * dt))
        };
        Self {
            linear_velocity: if linear_velocity.is_finite() {
                linear_velocity
            } else {
                Vec3::ZERO
            },
            angular_velocity: if angular_velocity.is_finite() {
                angular_velocity
            } else {
                Vec3::ZERO
            },
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeaponEntitySockets {
    pub muzzle: Option<WeaponSocketPose>,
    /// Rear-sight pose. `rotation * +Z` is the rendered rear->front sight axis.
    pub sight: Option<WeaponSocketPose>,
    pub casing_ejection: Option<WeaponSocketPose>,
}

/// Latest world-space muzzle pose published by the equipped-weapon presentation.
///
/// Compatibility projection for gameplay systems that still read the owner entity. New weapon
/// systems should consume `EquippedWeaponEntity -> WeaponEntitySockets` instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquippedWeaponMuzzle {
    pub position: Vec3,
    pub forward: Vec3,
}

impl EquippedWeaponMuzzle {
    #[inline]
    pub fn new(position: Vec3, forward: Vec3) -> Option<Self> {
        let forward = forward.normalize_or_zero();
        (position.is_finite() && forward.length_squared() > 1.0e-8)
            .then_some(Self { position, forward })
    }
}

/// Latest world-space iron-sight line published by the rendered equipped weapon.
///
/// `position` is the rear sight and `forward` is the normalized rear->front sight axis. The weapon
/// entity socket is authoritative; this owner-side component is a compatibility projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquippedWeaponSight {
    pub position: Vec3,
    pub forward: Vec3,
}

impl EquippedWeaponSight {
    #[inline]
    pub fn new(position: Vec3, forward: Vec3) -> Option<Self> {
        let forward = forward.normalize_or_zero();
        (position.is_finite() && forward.length_squared() > 1.0e-8)
            .then_some(Self { position, forward })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemPickup {
    pub item: ItemId,
    pub quantity: u32,
    pub auto_equip: bool,
    pub destroy_when_empty: bool,
    pub enabled: bool,
}

impl ItemPickup {
    pub const fn new(item: ItemId, quantity: u32) -> Self {
        Self {
            item,
            quantity,
            auto_equip: false,
            destroy_when_empty: true,
            enabled: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryEventKind {
    ItemAdded,
    ItemRemoved,
    ItemReordered,
    StackSplit,
    StacksMerged,
    Equipped,
    Unequipped,
    ActiveSlotChanged,
    AmmoConsumed,
    ItemUsed,
    ItemDropped,
    PickupCollected,
    PickupRejected,
    LoadoutApplied,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InventoryEvent {
    pub kind: InventoryEventKind,
    pub owner: EntityId,
    pub item: ItemId,
    pub instance_id: Option<ItemInstanceId>,
    pub quantity: u32,
    pub slot: Option<EquipmentSlot>,
    pub world_entity: Option<EntityId>,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InventoryEventBus {
    pub events: VecDeque<InventoryEvent>,
}

impl InventoryEventBus {
    pub(super) fn emit(&mut self, event: InventoryEvent) {
        const CAPACITY: usize = 512;
        if self.events.len() == CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> Vec<InventoryEvent> {
        self.events.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_bus_keeps_exact_latest_capacity_in_order() {
        let mut world = World::new();
        let owner = world.spawn();
        let item = ItemId::from_name("test.inventory.event").expect("valid test item");
        let mut bus = InventoryEventBus::default();

        for sequence in 0..2_000u32 {
            bus.emit(InventoryEvent {
                kind: InventoryEventKind::ItemAdded,
                owner,
                item,
                instance_id: None,
                quantity: sequence,
                slot: None,
                world_entity: None,
                message: String::new(),
            });
        }

        assert_eq!(bus.events.len(), 512);
        assert_eq!(bus.events.front().map(|event| event.quantity), Some(1_488));
        assert_eq!(bus.events.back().map(|event| event.quantity), Some(1_999));
        assert!(bus.events.iter().map(|event| event.quantity).is_sorted());
    }
}
