#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub u64);

pub const SHARED_UNARMED_WEAPON_ITEM_NAME: &str = "weapon.unarmed";

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

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

impl ItemInstanceId {
    /// Virtual, non-inventory weapon instance representing the character's own body/hands.
    /// Inventory allocation already rejects zero, so this identity cannot alias a real item.
    pub const UNARMED: Self = Self(0);

    #[inline]
    pub const fn is_unarmed(self) -> bool {
        self.0 == 0
    }
}

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
