use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemCatalog {
    definitions: BTreeMap<ItemId, ItemDefinition>,
    names: BTreeMap<String, ItemId>,
}

impl ItemCatalog {
    pub fn register(&mut self, definition: ItemDefinition) -> Result<ItemId, String> {
        if let Some(existing) = self.definitions.get(&definition.id) {
            if existing.name != definition.name {
                return Err(format!(
                    "item id collision: '{}' and '{}' resolve to {:016x}",
                    existing.name, definition.name, definition.id.0
                ));
            }
        }
        if let Some(existing_id) = self.names.get(&definition.name) {
            if *existing_id != definition.id {
                return Err(format!(
                    "item name '{}' already belongs to a different id",
                    definition.name
                ));
            }
        }
        let id = definition.id;
        self.names.insert(definition.name.clone(), id);
        self.definitions.insert(id, definition);
        Ok(id)
    }

    #[inline]
    pub fn get(&self, id: ItemId) -> Option<&ItemDefinition> {
        self.definitions.get(&id)
    }

    pub fn find(&self, name: &str) -> Option<&ItemDefinition> {
        let normalized = normalize_item_name(name)?;
        let id = self.names.get(&normalized)?;
        self.definitions.get(id)
    }

    pub fn definitions(&self) -> impl Iterator<Item = &ItemDefinition> {
        self.definitions.values()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn fps_defaults() -> Self {
        let ammo = ItemDefinition::stackable(
            DEFAULT_RIFLE_AMMO_NAME,
            "Standard Rifle Ammunition",
            ItemKind::Ammo,
            240,
            0.012,
        )
        .expect("valid built-in ammo definition");
        let ammo_id = ammo.id;
        let rifle = ItemDefinition::weapon(
            DEFAULT_RIFLE_ITEM_NAME,
            "Standard Service Rifle",
            EquipmentSlot::Primary,
            HitscanWeaponTuning::default(),
            ammo_id,
            3.6,
        )
        .expect("valid built-in weapon definition");
        let medkit = ItemDefinition::consumable(
            DEFAULT_MEDKIT_ITEM_NAME,
            "Field Medkit",
            5,
            0.75,
            ItemUseEffect::Heal { amount: 45.0 },
        )
        .expect("valid built-in consumable definition");

        let mut catalog = Self::default();
        catalog.register(ammo).expect("unique built-in ammo");
        catalog.register(rifle).expect("unique built-in rifle");
        catalog.register(medkit).expect("unique built-in medkit");
        catalog
    }
}
