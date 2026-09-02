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
    pub ammo_profile: Option<AuthoredAmmoDefinition>,
    pub weapon_components: Option<AuthoredWeaponComponentGraphDefinition>,
    pub weapon_animation: Option<AuthoredWeaponAnimationDefinition>,
    pub weapon_audio: Option<AuthoredWeaponAudioDefinition>,
    pub weapon_vfx: Option<AuthoredWeaponVfxDefinition>,
    pub weapon_presentation: Option<AuthoredWeaponPresentationDefinition>,
    pub weapon_casing: Option<AuthoredWeaponCasingDefinition>,
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
            ammo_profile: None,
            weapon_components: None,
            weapon_animation: None,
            weapon_audio: None,
            weapon_vfx: None,
            weapon_presentation: None,
            weapon_casing: None,
            use_effect: None,
            world: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredAmmoDefinition {
    pub caliber: String,
    pub projectile_type: String,
    pub projectile_mass_kg: f32,
    pub muzzle_velocity_mps: f32,
    pub penetration_energy_j: f32,
    pub max_penetration_m: f32,
    pub drag_coefficient: f32,
    pub damage_multiplier: f32,
    pub impulse_multiplier: f32,
    pub falloff_start_m: f32,
    pub falloff_end_m: f32,
    pub falloff_min_multiplier: f32,
    pub tracer: bool,
    pub impact_profile: String,
}

impl Default for AuthoredAmmoDefinition {
    fn default() -> Self {
        let runtime = AmmoDefinition::default();
        Self {
            caliber: runtime.caliber,
            projectile_type: "instant".to_owned(),
            projectile_mass_kg: runtime.projectile_mass_kg,
            muzzle_velocity_mps: runtime.muzzle_velocity_mps,
            penetration_energy_j: runtime.penetration_energy_j,
            max_penetration_m: runtime.max_penetration_m,
            drag_coefficient: runtime.drag_coefficient,
            damage_multiplier: runtime.damage_multiplier,
            impulse_multiplier: runtime.impulse_multiplier,
            falloff_start_m: runtime.falloff_start_m,
            falloff_end_m: runtime.falloff_end_m,
            falloff_min_multiplier: runtime.falloff_min_multiplier,
            tracer: runtime.tracer,
            impact_profile: String::new(),
        }
    }
}

impl AuthoredAmmoDefinition {
    pub(super) fn compile(&self) -> Result<AmmoDefinition, String> {
        let projectile_type = match self.projectile_type.trim().to_ascii_lowercase().as_str() {
            "" | "instant" | "hitscan" | "ballistic_ray" => AmmoProjectileType::Instant,
            "physical" | "projectile" => AmmoProjectileType::Physical,
            other => return Err(format!("unsupported ammo projectile_type '{other}'")),
        };
        Ok(AmmoDefinition {
            caliber: self.caliber.clone(),
            projectile_type,
            projectile_mass_kg: self.projectile_mass_kg,
            muzzle_velocity_mps: self.muzzle_velocity_mps,
            penetration_energy_j: self.penetration_energy_j,
            max_penetration_m: self.max_penetration_m,
            drag_coefficient: self.drag_coefficient,
            damage_multiplier: self.damage_multiplier,
            impulse_multiplier: self.impulse_multiplier,
            falloff_start_m: self.falloff_start_m,
            falloff_end_m: self.falloff_end_m,
            falloff_min_multiplier: self.falloff_min_multiplier,
            tracer: self.tracer,
            impact_profile: (!self.impact_profile.trim().is_empty())
                .then(|| self.impact_profile.trim().to_owned()),
        }
        .sanitized())
    }
}
