#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponComponentModifiers {
    pub accuracy_multiplier: f32,
    pub recoil_multiplier: f32,
    pub damage_multiplier: f32,
    pub falloff_multiplier: f32,
    pub muzzle_velocity_multiplier: f32,
    pub penetration_multiplier: f32,
    pub audio_gain_multiplier: f32,
    pub presentation_offset_local: [f32; 3],
}

impl Default for WeaponComponentModifiers {
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

impl WeaponComponentModifiers {
    pub fn sanitized(self) -> Self {
        Self {
            accuracy_multiplier: finite_or(self.accuracy_multiplier, 1.0).clamp(0.05, 20.0),
            recoil_multiplier: finite_or(self.recoil_multiplier, 1.0).clamp(0.0, 20.0),
            damage_multiplier: finite_or(self.damage_multiplier, 1.0).clamp(0.0, 20.0),
            falloff_multiplier: finite_or(self.falloff_multiplier, 1.0).clamp(0.0, 20.0),
            muzzle_velocity_multiplier: finite_or(self.muzzle_velocity_multiplier, 1.0)
                .clamp(0.05, 20.0),
            penetration_multiplier: finite_or(self.penetration_multiplier, 1.0).clamp(0.0, 20.0),
            audio_gain_multiplier: finite_or(self.audio_gain_multiplier, 1.0).clamp(0.0, 4.0),
            presentation_offset_local: self
                .presentation_offset_local
                .map(|value| finite_or(value, 0.0).clamp(-2.0, 2.0)),
        }
    }

    pub fn combine(self, other: Self) -> Self {
        let a = self.sanitized();
        let b = other.sanitized();
        Self {
            accuracy_multiplier: a.accuracy_multiplier * b.accuracy_multiplier,
            recoil_multiplier: a.recoil_multiplier * b.recoil_multiplier,
            damage_multiplier: a.damage_multiplier * b.damage_multiplier,
            falloff_multiplier: a.falloff_multiplier * b.falloff_multiplier,
            muzzle_velocity_multiplier: a.muzzle_velocity_multiplier * b.muzzle_velocity_multiplier,
            penetration_multiplier: a.penetration_multiplier * b.penetration_multiplier,
            audio_gain_multiplier: a.audio_gain_multiplier * b.audio_gain_multiplier,
            presentation_offset_local: [
                a.presentation_offset_local[0] + b.presentation_offset_local[0],
                a.presentation_offset_local[1] + b.presentation_offset_local[1],
                a.presentation_offset_local[2] + b.presentation_offset_local[2],
            ],
        }
        .sanitized()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponComponentDefinition {
    pub id: String,
    pub slot: String,
    pub model_ref: Option<String>,
    pub audio_override: Option<String>,
    pub muzzle_vfx_override: Option<String>,
    pub tracer_vfx_override: Option<String>,
    /// Open-ended stat graph contributions. This supplements the compatibility typed modifiers
    /// and lets authored attachments affect newly registered weapon stats without schema growth.
    pub stat_modifiers: WeaponStatModifierStack,
    pub modifiers: WeaponComponentModifiers,
}

impl WeaponComponentDefinition {
    pub fn sanitized(mut self) -> Self {
        self.id = self.id.trim().to_ascii_lowercase();
        self.slot = self.slot.trim().to_ascii_lowercase();
        let clean = |value: Option<String>| {
            value
                .map(|value| value.trim().replace('\\', "/"))
                .filter(|value| !value.is_empty())
        };
        self.model_ref = clean(self.model_ref);
        self.audio_override = clean(self.audio_override);
        self.muzzle_vfx_override = clean(self.muzzle_vfx_override);
        self.tracer_vfx_override = clean(self.tracer_vfx_override);
        self.stat_modifiers = self.stat_modifiers.sanitized();
        self.modifiers = self.modifiers.sanitized();
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponComponentPointDefinition {
    pub id: String,
    pub attach_joint: String,
    pub allowed_components: Vec<String>,
}

impl WeaponComponentPointDefinition {
    pub fn sanitized(mut self) -> Self {
        self.id = self.id.trim().to_ascii_lowercase();
        self.attach_joint = self.attach_joint.trim().to_ascii_lowercase();
        self.allowed_components = self
            .allowed_components
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
        self.allowed_components.sort();
        self.allowed_components.dedup();
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponComponentGraphDefinition {
    pub points: Vec<WeaponComponentPointDefinition>,
    pub components: BTreeMap<String, WeaponComponentDefinition>,
    pub default_installed: BTreeMap<String, String>,
}

impl WeaponComponentGraphDefinition {
    pub fn sanitized(mut self) -> Self {
        self.points = self
            .points
            .into_iter()
            .map(WeaponComponentPointDefinition::sanitized)
            .filter(|point| !point.id.is_empty())
            .collect();
        self.points.sort_by(|a, b| a.id.cmp(&b.id));
        self.points.dedup_by(|a, b| a.id == b.id);
        self.components = self
            .components
            .into_values()
            .map(WeaponComponentDefinition::sanitized)
            .filter(|component| !component.id.is_empty() && !component.slot.is_empty())
            .map(|component| (component.id.clone(), component))
            .collect();
        self.default_installed = self
            .default_installed
            .into_iter()
            .map(|(slot, component)| {
                (
                    slot.trim().to_ascii_lowercase(),
                    component.trim().to_ascii_lowercase(),
                )
            })
            .filter(|(slot, component)| !slot.is_empty() && !component.is_empty())
            .collect();
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        let graph = self.clone().sanitized();
        for (slot, component_id) in &graph.default_installed {
            let point = graph
                .points
                .iter()
                .find(|point| &point.id == slot)
                .ok_or_else(|| format!("component default references unknown slot '{slot}'"))?;
            let component = graph.components.get(component_id).ok_or_else(|| {
                format!("component default references unknown component '{component_id}'")
            })?;
            if component.slot != *slot
                || (!point.allowed_components.is_empty()
                    && !point.allowed_components.contains(component_id))
            {
                return Err(format!(
                    "component '{component_id}' is not allowed in slot '{slot}'"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponComponentInstance {
    pub component_id: String,
    pub active: bool,
}
