use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub u64);

impl ItemId {
    pub fn from_name(name: &str) -> Option<Self> {
        let normalized = normalize_item_name(name)?;
        Some(Self(stable_hash64(normalized.as_bytes())))
    }

    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemInstanceId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    #[default]
    Generic,
    Weapon,
    Ammo,
    Consumable,
    Component,
    Quest,
    Key,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EquipmentSlot {
    #[default]
    Primary,
    Secondary,
    Sidearm,
    Melee,
    Throwable,
    Gadget,
    Utility1,
    Utility2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ItemUseEffect {
    #[default]
    None,
    Heal {
        amount: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldItemDefinition {
    pub model_ref: Option<String>,
    pub material_library_ref: Option<String>,
    pub fallback_primitive: PrimitiveId,
    pub scale: [f32; 3],
    pub color: [f32; 4],
    pub pickup_half_extents: [f32; 3],
    pub respawn_seconds: f32,
}

impl WorldItemDefinition {
    pub fn for_kind(kind: ItemKind) -> Self {
        match kind {
            ItemKind::Weapon => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.42, 0.12, 0.10],
                color: [0.22, 0.27, 0.32, 1.0],
                pickup_half_extents: [0.42, 0.12, 0.10],
                respawn_seconds: 0.0,
            },
            ItemKind::Ammo => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.18, 0.10, 0.14],
                color: [0.72, 0.52, 0.18, 1.0],
                pickup_half_extents: [0.18, 0.10, 0.14],
                respawn_seconds: 0.0,
            },
            ItemKind::Consumable => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_CUBE,
                scale: [0.20, 0.14, 0.22],
                color: [0.74, 0.18, 0.22, 1.0],
                pickup_half_extents: [0.20, 0.14, 0.22],
                respawn_seconds: 0.0,
            },
            ItemKind::Key | ItemKind::Quest => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_TORUS,
                scale: [0.16, 0.16, 0.05],
                color: [0.25, 0.70, 0.92, 1.0],
                pickup_half_extents: [0.16, 0.16, 0.08],
                respawn_seconds: 0.0,
            },
            ItemKind::Generic | ItemKind::Component => Self {
                model_ref: None,
                material_library_ref: None,
                fallback_primitive: primitive_builtins::ID_SPHERE_UV,
                scale: [0.16, 0.16, 0.16],
                color: [0.48, 0.55, 0.65, 1.0],
                pickup_half_extents: [0.16, 0.16, 0.16],
                respawn_seconds: 0.0,
            },
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.scale = sanitize_positive_vec3(self.scale, 0.01, 20.0);
        self.pickup_half_extents = sanitize_positive_vec3(self.pickup_half_extents, 0.01, 10.0);
        self.color = self.color.map(|value| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                1.0
            }
        });
        self.respawn_seconds = sanitize_non_negative(self.respawn_seconds).min(86_400.0);
        self.model_ref = self
            .model_ref
            .take()
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self.material_library_ref = self
            .material_library_ref
            .take()
            .map(|value| value.trim().replace('\\', "/"))
            .filter(|value| !value.is_empty());
        self
    }
}

impl Default for WorldItemDefinition {
    fn default() -> Self {
        Self::for_kind(ItemKind::Generic)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldItemPresentation {
    pub visual_entity: EntityId,
    pub model_ref: Option<String>,
    pub fallback_primitive: PrimitiveId,
    pub scale: Vec3,
    pub color: [f32; 4],
    pub pickup_half_extents: Vec3,
    /// True only after the authored model/material hierarchy has been admitted.
    /// Authored items intentionally do not expose the generic fallback primitive while false.
    pub authored_visual_admitted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldItemVisualPart {
    pub owner: EntityId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldItemRuntime {
    pub persistent_id: u64,
    pub spawn_position: Vec3,
    pub original_quantity: u32,
    pub respawn_seconds: f32,
    pub respawn_remaining: f32,
    pub pickup_cooldown_remaining: f32,
    pub dropped: bool,
}

impl WorldItemRuntime {
    #[inline]
    pub fn persistent_source(
        persistent_id: u64,
        spawn_position: Vec3,
        quantity: u32,
        respawn_seconds: f32,
    ) -> Self {
        Self {
            persistent_id,
            spawn_position,
            original_quantity: quantity.max(1),
            respawn_seconds: sanitize_non_negative(respawn_seconds),
            respawn_remaining: 0.0,
            pickup_cooldown_remaining: 0.0,
            dropped: false,
        }
    }

    #[inline]
    pub fn dropped(persistent_id: u64, spawn_position: Vec3, quantity: u32) -> Self {
        Self {
            persistent_id,
            spawn_position,
            original_quantity: quantity.max(1),
            respawn_seconds: 0.0,
            respawn_remaining: 0.0,
            pickup_cooldown_remaining: 0.25,
            dropped: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponFireMode {
    #[default]
    SemiAuto,
    Automatic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponItemDefinition {
    pub tuning: HitscanWeaponTuning,
    pub ammo_item: ItemId,
    pub fire_mode: WeaponFireMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WeaponAudioAction {
    #[default]
    Fire,
    ReloadStart,
    ReloadComplete,
    Equip,
    Unequip,
    Empty,
    ShellEject,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponAudioDefinition {
    pub fire: Option<String>,
    pub reload_start: Option<String>,
    pub reload_complete: Option<String>,
    pub equip: Option<String>,
    pub unequip: Option<String>,
    pub empty: Option<String>,
    pub shell_eject: Option<String>,
}

impl WeaponAudioDefinition {
    pub fn sanitized(mut self) -> Self {
        fn clean(value: Option<String>) -> Option<String> {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        }
        self.fire = clean(self.fire);
        self.reload_start = clean(self.reload_start);
        self.reload_complete = clean(self.reload_complete);
        self.equip = clean(self.equip);
        self.unequip = clean(self.unequip);
        self.empty = clean(self.empty);
        self.shell_eject = clean(self.shell_eject);
        self
    }

    #[inline]
    pub fn clip(&self, action: WeaponAudioAction) -> Option<&str> {
        match action {
            WeaponAudioAction::Fire => self.fire.as_deref(),
            WeaponAudioAction::ReloadStart => self.reload_start.as_deref(),
            WeaponAudioAction::ReloadComplete => self.reload_complete.as_deref(),
            WeaponAudioAction::Equip => self.equip.as_deref(),
            WeaponAudioAction::Unequip => self.unequip.as_deref(),
            WeaponAudioAction::Empty => self.empty.as_deref(),
            WeaponAudioAction::ShellEject => self.shell_eject.as_deref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemDefinition {
    pub id: ItemId,
    pub name: String,
    pub definition_ref: Option<String>,
    pub display_name: String,
    pub description: String,
    pub icon_ref: Option<String>,
    pub tags: Vec<String>,
    pub kind: ItemKind,
    pub max_stack: u32,
    pub unit_weight: f32,
    pub equipment_slot: Option<EquipmentSlot>,
    pub weapon: Option<WeaponItemDefinition>,
    pub weapon_audio: WeaponAudioDefinition,
    pub use_effect: ItemUseEffect,
    pub world: WorldItemDefinition,
}

impl ItemDefinition {
    pub fn stackable(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        kind: ItemKind,
        max_stack: u32,
        unit_weight: f32,
    ) -> Result<Self, String> {
        let name = normalize_item_name(name.as_ref())
            .ok_or_else(|| "item name must contain at least one valid character".to_owned())?;
        let id = ItemId(stable_hash64(name.as_bytes()));
        Ok(Self {
            id,
            name,
            definition_ref: None,
            display_name: display_name.into(),
            description: String::new(),
            icon_ref: None,
            tags: Vec::new(),
            kind,
            max_stack: max_stack.clamp(1, 1_000_000),
            unit_weight: sanitize_non_negative(unit_weight),
            equipment_slot: None,
            weapon: None,
            weapon_audio: WeaponAudioDefinition::default(),
            use_effect: ItemUseEffect::None,
            world: WorldItemDefinition::for_kind(kind),
        })
    }

    pub fn weapon(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        slot: EquipmentSlot,
        tuning: HitscanWeaponTuning,
        ammo_item: ItemId,
        fire_mode: WeaponFireMode,
        unit_weight: f32,
    ) -> Result<Self, String> {
        let mut item = Self::stackable(name, display_name, ItemKind::Weapon, 1, unit_weight)?;
        item.equipment_slot = Some(slot);
        item.weapon = Some(WeaponItemDefinition {
            tuning: tuning.sanitized(),
            ammo_item,
            fire_mode,
        });
        Ok(item)
    }

    #[inline]
    pub fn with_weapon_audio(mut self, audio: WeaponAudioDefinition) -> Self {
        self.weapon_audio = audio.sanitized();
        self
    }

    #[inline]
    pub fn with_definition_ref(mut self, definition_ref: impl Into<String>) -> Self {
        let value = definition_ref.into().trim().replace('\\', "/");
        self.definition_ref = (!value.is_empty()).then_some(value);
        self
    }

    #[inline]
    pub fn with_world_definition(mut self, world: WorldItemDefinition) -> Self {
        self.world = world.sanitized();
        self
    }

    #[inline]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    #[inline]
    pub fn with_icon(mut self, icon_ref: impl Into<String>) -> Self {
        let icon_ref = icon_ref.into();
        self.icon_ref = (!icon_ref.trim().is_empty()).then_some(icon_ref);
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags
            .into_iter()
            .map(Into::into)
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        self.tags.sort();
        self.tags.dedup();
        self
    }

    pub fn consumable(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        max_stack: u32,
        unit_weight: f32,
        effect: ItemUseEffect,
    ) -> Result<Self, String> {
        let mut item = Self::stackable(
            name,
            display_name,
            ItemKind::Consumable,
            max_stack,
            unit_weight,
        )?;
        item.use_effect = match effect {
            ItemUseEffect::Heal { amount } => ItemUseEffect::Heal {
                amount: sanitize_non_negative(amount),
            },
            ItemUseEffect::None => ItemUseEffect::None,
        };
        Ok(item)
    }
}
