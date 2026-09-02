#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentModifiers {
    pub accuracy_multiplier: f32,
    pub recoil_multiplier: f32,
    pub damage_multiplier: f32,
    pub falloff_multiplier: f32,
    pub muzzle_velocity_multiplier: f32,
    pub penetration_multiplier: f32,
    pub audio_gain_multiplier: f32,
    pub presentation_offset_local: [f32; 3],
}

impl Default for AuthoredWeaponComponentModifiers {
    fn default() -> Self {
        Self {
            accuracy_multiplier: 1.0,
            recoil_multiplier: 1.0,
            damage_multiplier: 1.0,
            falloff_multiplier: 1.0,
            muzzle_velocity_multiplier: 1.0,
            penetration_multiplier: 1.0,
            audio_gain_multiplier: 1.0,
            presentation_offset_local: [0.0; 3],
        }
    }
}

impl AuthoredWeaponComponentModifiers {
    fn compile(&self) -> WeaponComponentModifiers {
        WeaponComponentModifiers {
            accuracy_multiplier: self.accuracy_multiplier,
            recoil_multiplier: self.recoil_multiplier,
            damage_multiplier: self.damage_multiplier,
            falloff_multiplier: self.falloff_multiplier,
            muzzle_velocity_multiplier: self.muzzle_velocity_multiplier,
            penetration_multiplier: self.penetration_multiplier,
            audio_gain_multiplier: self.audio_gain_multiplier,
            presentation_offset_local: self.presentation_offset_local,
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentDefinition {
    pub id: String,
    pub slot: String,
    pub model_ref: String,
    pub audio_override: String,
    pub muzzle_vfx_override: String,
    pub tracer_vfx_override: String,
    pub stat_modifiers: Vec<AuthoredWeaponStatModifier>,
    pub modifiers: AuthoredWeaponComponentModifiers,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentPointDefinition {
    pub id: String,
    pub attach_joint: String,
    pub allowed_components: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AuthoredWeaponComponentGraphDefinition {
    pub points: Vec<AuthoredWeaponComponentPointDefinition>,
    pub components: Vec<AuthoredWeaponComponentDefinition>,
    pub default_installed: BTreeMap<String, String>,
}

impl AuthoredWeaponComponentGraphDefinition {
    pub(super) fn compile(&self) -> Result<WeaponComponentGraphDefinition, String> {
        let graph = WeaponComponentGraphDefinition {
            points: self
                .points
                .iter()
                .map(|point| WeaponComponentPointDefinition {
                    id: point.id.clone(),
                    attach_joint: point.attach_joint.clone(),
                    allowed_components: point.allowed_components.clone(),
                })
                .collect(),
            components: self
                .components
                .iter()
                .map(|component| -> Result<_, String> {
                    let id = component.id.trim().to_ascii_lowercase();
                    Ok((
                        id.clone(),
                        WeaponComponentDefinition {
                            id,
                            slot: component.slot.clone(),
                            model_ref: (!component.model_ref.trim().is_empty())
                                .then(|| component.model_ref.clone()),
                            audio_override: (!component.audio_override.trim().is_empty())
                                .then(|| component.audio_override.clone()),
                            muzzle_vfx_override: (!component.muzzle_vfx_override.trim().is_empty())
                                .then(|| component.muzzle_vfx_override.clone()),
                            tracer_vfx_override: (!component.tracer_vfx_override.trim().is_empty())
                                .then(|| component.tracer_vfx_override.clone()),
                            stat_modifiers: crate::weapon_profiles::compile_weapon_stat_stack(
                                &component.stat_modifiers,
                            )?,
                            modifiers: component.modifiers.compile(),
                        },
                    ))
                })
                .collect::<Result<_, _>>()?,
            default_installed: self.default_installed.clone(),
        }
        .sanitized();
        graph.validate()?;
        Ok(graph)
    }
}
