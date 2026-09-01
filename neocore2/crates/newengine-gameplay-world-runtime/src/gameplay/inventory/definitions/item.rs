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
    /// Project-authored open-ended weapon presentation family (for example `pistol` or `rifle`).
    /// Gameplay weapon mechanics remain keyed by `WeaponType`; presentation uses this value only
    /// to select character-authored equipment pose sets without hard-coding weapon classes.
    pub weapon_class: Option<String>,
    /// Present only for `ItemKind::Ammo`; firearm mechanics reference ammo by item identity.
    pub ammo_profile: Option<AmmoDefinition>,
    pub weapon_components: WeaponComponentGraphDefinition,
    pub weapon_presentation: WeaponPresentationDefinition,
    pub weapon_animation: WeaponAnimationDefinition,
    pub weapon_audio: WeaponAudioDefinition,
    pub weapon_vfx: WeaponVfxDefinition,
    pub weapon_casing: WeaponCasingDefinition,
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
            weapon_class: None,
            ammo_profile: None,
            weapon_components: WeaponComponentGraphDefinition::default(),
            weapon_presentation: WeaponPresentationDefinition::default(),
            weapon_animation: WeaponAnimationDefinition::default(),
            weapon_audio: WeaponAudioDefinition::default(),
            weapon_vfx: WeaponVfxDefinition::default(),
            weapon_casing: WeaponCasingDefinition::default(),
            use_effect: ItemUseEffect::None,
            world: WorldItemDefinition::for_kind(kind),
        })
    }

    pub fn typed_weapon(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        slot: Option<EquipmentSlot>,
        weapon: WeaponItemDefinition,
        unit_weight: f32,
    ) -> Result<Self, String> {
        let mut item = Self::stackable(name, display_name, ItemKind::Weapon, 1, unit_weight)?;
        item.equipment_slot = slot;
        item.weapon = Some(weapon);
        Ok(item)
    }

    /// Backward-compatible firearm constructor. Concrete weapons remain project-authored; this
    /// helper only constructs the engine's Firearm weapon type.
    pub fn weapon(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        slot: EquipmentSlot,
        tuning: HitscanWeaponTuning,
        ammo_item: ItemId,
        fire_mode: WeaponFireMode,
        unit_weight: f32,
    ) -> Result<Self, String> {
        Self::typed_weapon(
            name,
            display_name,
            Some(slot),
            WeaponItemDefinition::firearm(
                WeaponType::Firearm.default_rank(),
                tuning,
                ammo_item,
                fire_mode,
            ),
            unit_weight,
        )
    }

    pub fn melee_weapon(
        name: impl AsRef<str>,
        display_name: impl Into<String>,
        slot: EquipmentSlot,
        rank: u16,
        tuning: MeleeWeaponTuning,
        unit_weight: f32,
    ) -> Result<Self, String> {
        Self::typed_weapon(
            name,
            display_name,
            Some(slot),
            WeaponItemDefinition::melee(rank, tuning),
            unit_weight,
        )
    }

    #[inline]
    pub fn with_ammo_profile(mut self, ammo: AmmoDefinition) -> Self {
        self.ammo_profile = Some(ammo.sanitized());
        self
    }

    pub fn with_weapon_components(
        mut self,
        graph: WeaponComponentGraphDefinition,
    ) -> Result<Self, String> {
        let graph = graph.sanitized();
        graph.validate()?;
        self.weapon_components = graph;
        Ok(self)
    }

    #[inline]
    pub fn with_weapon_class(mut self, weapon_class: impl AsRef<str>) -> Self {
        let normalized = weapon_class
            .as_ref()
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_");
        self.weapon_class = (!normalized.is_empty()).then_some(normalized);
        self
    }

    #[inline]
    pub fn with_weapon_presentation(mut self, presentation: WeaponPresentationDefinition) -> Self {
        self.weapon_presentation = presentation.sanitized();
        self
    }

    pub fn with_weapon_animation(mut self, animation: WeaponAnimationDefinition) -> Self {
        self.weapon_animation = animation.sanitized();
        self
    }

    #[inline]
    pub fn with_weapon_audio(mut self, audio: WeaponAudioDefinition) -> Self {
        self.weapon_audio = audio.sanitized();
        self
    }

    #[inline]
    pub fn with_weapon_vfx(mut self, vfx: WeaponVfxDefinition) -> Self {
        self.weapon_vfx = vfx.sanitized();
        self
    }

    #[inline]
    pub fn with_weapon_casing(mut self, casing: WeaponCasingDefinition) -> Self {
        self.weapon_casing = casing.sanitized();
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
