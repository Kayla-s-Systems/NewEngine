use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredItemPackage {
    pub schema: String,
    pub version: u32,
    pub items: Vec<AuthoredItemDefinition>,
    pub loadouts: Vec<AuthoredLoadoutDefinition>,
}

impl Default for AuthoredItemPackage {
    fn default() -> Self {
        Self {
            schema: AUTHORED_ITEM_PACKAGE_SCHEMA.to_owned(),
            version: AUTHORED_ITEM_PACKAGE_VERSION,
            items: Vec::new(),
            loadouts: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredItemDefinition {
    pub id: String,
    pub definition_ref: String,
    pub display_name: String,
    pub description: String,
    pub icon: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub max_stack: u32,
    pub unit_weight: f32,
    pub equipment_slot: String,
    pub weapon: Option<AuthoredWeaponDefinition>,
    pub use_effect: Option<AuthoredUseEffect>,
    pub world: Option<AuthoredWorldItemDefinition>,
}

impl Default for AuthoredItemDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            definition_ref: String::new(),
            display_name: String::new(),
            description: String::new(),
            icon: String::new(),
            tags: Vec::new(),
            kind: "generic".to_owned(),
            max_stack: 1,
            unit_weight: 0.0,
            equipment_slot: String::new(),
            weapon: None,
            use_effect: None,
            world: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponDefinition {
    pub ammo: String,
    pub fire_mode: String,
    pub magazine_capacity: u32,
    pub reserve_capacity: u32,
    pub fire_interval: f32,
    pub reload_duration: f32,
    pub damage: f32,
    pub range: f32,
    pub hip_spread_degrees: f32,
    pub aim_spread_degrees: f32,
    pub recoil_pitch_degrees: f32,
    pub recoil_yaw_degrees: f32,
    pub muzzle_forward_offset: f32,
}

impl Default for AuthoredWeaponDefinition {
    fn default() -> Self {
        let tuning = HitscanWeaponTuning::default();
        Self {
            ammo: String::new(),
            fire_mode: "semi_auto".to_owned(),
            magazine_capacity: tuning.magazine_capacity,
            reserve_capacity: tuning.reserve_capacity,
            fire_interval: tuning.fire_interval,
            reload_duration: tuning.reload_duration,
            damage: tuning.damage,
            range: tuning.range,
            hip_spread_degrees: tuning.hip_spread_radians.to_degrees(),
            aim_spread_degrees: tuning.aim_spread_radians.to_degrees(),
            recoil_pitch_degrees: tuning.recoil_pitch_radians.to_degrees(),
            recoil_yaw_degrees: tuning.recoil_yaw_radians.to_degrees(),
            muzzle_forward_offset: tuning.muzzle_forward_offset,
        }
    }
}

impl AuthoredWeaponDefinition {
    pub(super) fn fire_mode(&self) -> Result<WeaponFireMode, String> {
        match self
            .fire_mode
            .trim()
            .to_ascii_lowercase()
            .replace('-', "_")
            .as_str()
        {
            "" | "semi" | "semi_auto" | "semiauto" => Ok(WeaponFireMode::SemiAuto),
            "auto" | "automatic" | "full_auto" | "fullauto" => Ok(WeaponFireMode::Automatic),
            other => Err(format!("unsupported weapon fire_mode '{other}'")),
        }
    }

    pub(super) fn tuning(&self) -> HitscanWeaponTuning {
        HitscanWeaponTuning {
            magazine_capacity: self.magazine_capacity,
            reserve_capacity: self.reserve_capacity,
            fire_interval: self.fire_interval,
            reload_duration: self.reload_duration,
            damage: self.damage,
            range: self.range,
            hip_spread_radians: self.hip_spread_degrees.to_radians(),
            aim_spread_radians: self.aim_spread_degrees.to_radians(),
            recoil_pitch_radians: self.recoil_pitch_degrees.to_radians(),
            recoil_yaw_radians: self.recoil_yaw_degrees.to_radians(),
            muzzle_forward_offset: self.muzzle_forward_offset,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredUseEffect {
    pub kind: String,
    pub amount: f32,
}

impl Default for AuthoredUseEffect {
    fn default() -> Self {
        Self {
            kind: "none".to_owned(),
            amount: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWorldItemDefinition {
    pub model: String,
    pub material_library: String,
    pub fallback_primitive: String,
    pub scale: [f32; 3],
    pub color_rgba: [f32; 4],
    pub pickup_half_extents: [f32; 3],
    pub respawn_seconds: f32,
}

impl Default for AuthoredWorldItemDefinition {
    fn default() -> Self {
        Self {
            model: String::new(),
            material_library: String::new(),
            fallback_primitive: "cube".to_owned(),
            scale: [0.2, 0.2, 0.2],
            color_rgba: [0.55, 0.60, 0.68, 1.0],
            pickup_half_extents: [0.2, 0.2, 0.2],
            respawn_seconds: 0.0,
        }
    }
}

impl AuthoredWorldItemDefinition {
    pub(super) fn compile(&self, kind: ItemKind) -> Result<WorldItemDefinition, String> {
        let fallback_primitive = match self.fallback_primitive.trim().to_ascii_lowercase().as_str()
        {
            "" | "cube" => primitive_builtins::ID_CUBE,
            "sphere" | "sphere_uv" => primitive_builtins::ID_SPHERE_UV,
            "cylinder" => primitive_builtins::ID_CYLINDER,
            "capsule" => primitive_builtins::ID_CAPSULE,
            "cone" => primitive_builtins::ID_CONE,
            "torus" => primitive_builtins::ID_TORUS,
            "disc" => primitive_builtins::ID_DISC,
            other => return Err(format!("unsupported world fallback primitive '{other}'")),
        };
        let mut definition = WorldItemDefinition::for_kind(kind);
        definition.model_ref =
            (!self.model.trim().is_empty()).then(|| self.model.trim().to_owned());
        definition.material_library_ref = (!self.material_library.trim().is_empty())
            .then(|| self.material_library.trim().to_owned());
        definition.fallback_primitive = fallback_primitive;
        definition.scale = self.scale;
        definition.color = self.color_rgba;
        definition.pickup_half_extents = self.pickup_half_extents;
        definition.respawn_seconds = self.respawn_seconds;
        Ok(definition.sanitized())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredLoadoutDefinition {
    pub id: String,
    pub display_name: String,
    pub clear_existing: bool,
    pub entries: Vec<AuthoredLoadoutEntry>,
}

impl Default for AuthoredLoadoutDefinition {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            clear_existing: true,
            entries: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct AuthoredLoadoutEntry {
    pub item: String,
    pub quantity: u32,
    pub equip: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledItemPackage {
    pub catalog: ItemCatalog,
    pub loadouts: InventoryLoadoutCatalog,
}
