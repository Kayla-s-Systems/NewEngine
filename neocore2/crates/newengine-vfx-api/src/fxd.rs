use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::VfxPriority;

pub const FXD_SCHEMA_V1: &str = "newengine.fxd.v1";
pub const FXD_VERSION_V1: u32 = 1;

/// Project-authored effect dictionary. The engine owns validation/execution only;
/// effect composition, texture references and all visual tuning live in project data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxdDictionaryV1 {
    pub schema: String,
    pub version: u32,
    pub textures: Vec<FxdTextureV1>,
    pub effects: Vec<FxdEffectV1>,
}

impl Default for FxdDictionaryV1 {
    fn default() -> Self {
        Self {
            schema: FXD_SCHEMA_V1.to_owned(),
            version: FXD_VERSION_V1,
            textures: Vec::new(),
            effects: Vec::new(),
        }
    }
}

impl FxdDictionaryV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FXD_SCHEMA_V1 {
            return Err(format!(
                "FXD schema mismatch: got='{}' expected='{}'",
                self.schema, FXD_SCHEMA_V1
            ));
        }
        if self.version != FXD_VERSION_V1 {
            return Err(format!(
                "FXD version mismatch: got={} expected={}",
                self.version, FXD_VERSION_V1
            ));
        }
        if self.effects.is_empty() {
            return Err("FXD dictionary must contain at least one effect".to_owned());
        }

        let mut texture_ids = BTreeSet::new();
        for texture in &self.textures {
            texture.validate()?;
            let key = texture.id.trim().to_ascii_lowercase();
            if !texture_ids.insert(key.clone()) {
                return Err(format!("FXD duplicate texture id '{key}'"));
            }
        }

        let mut effect_ids = BTreeSet::new();
        for effect in &self.effects {
            effect.validate(&texture_ids)?;
            let key = effect.id.trim().to_ascii_lowercase();
            if !effect_ids.insert(key.clone()) {
                return Err(format!("FXD duplicate effect id '{key}'"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxdTextureV1 {
    /// Dictionary-local texture id referenced by layers.
    pub id: String,
    /// Project/VFS logical texture entry, e.g. `textures/vfx/weapon.ytd@muzzle_main`.
    pub source: String,
}

impl FxdTextureV1 {
    fn validate(&self) -> Result<(), String> {
        let id = self.id.trim();
        let source = self.source.trim();
        if id.is_empty() || id.len() > 128 {
            return Err("FXD texture id must contain 1..=128 bytes".to_owned());
        }
        if source.is_empty() || source.len() > 1024 {
            return Err(format!("FXD texture '{id}' has invalid source"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxdEffectV1 {
    /// Dictionary-local effect id. Runtime canonicalizes it as `path.fxd@id`.
    pub id: String,
    pub priority: VfxPriority,
    pub layers: Vec<FxdLayerV1>,
}

impl Default for FxdEffectV1 {
    fn default() -> Self {
        Self {
            id: String::new(),
            priority: VfxPriority::Normal,
            layers: Vec::new(),
        }
    }
}

impl FxdEffectV1 {
    fn validate(&self, texture_ids: &BTreeSet<String>) -> Result<(), String> {
        let id = self.id.trim();
        if id.is_empty() || id.len() > 256 {
            return Err("FXD effect id must contain 1..=256 bytes".to_owned());
        }
        if self.layers.is_empty() {
            return Err(format!("FXD effect '{id}' has no layers"));
        }
        for (index, layer) in self.layers.iter().enumerate() {
            layer
                .validate(texture_ids)
                .map_err(|error| format!("FXD effect '{id}' layer {index}: {error}"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxdLayerKindV1 {
    MuzzleFlash,
    MuzzleCore,
    Smoke,
    Tracer,
    Spark,
    ImpactDecal,
    Trail,
    Debris,
    #[default]
    Generic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxdRenderRoleV1 {
    #[default]
    Transparent,
    Decal,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxdAlignmentV1 {
    #[default]
    None,
    DirectionY,
    DirectionZ,
    NormalY,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxdBillboardModeV1 {
    #[default]
    CameraFacing,
    VelocityAligned,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxdLightV1 {
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FxdLayerV1 {
    Pulse {
        kind: FxdLayerKindV1,
        primitive: String,
        #[serde(default)]
        role: FxdRenderRoleV1,
        #[serde(default)]
        alignment: FxdAlignmentV1,
        #[serde(default)]
        texture: String,
        #[serde(default)]
        billboard: FxdBillboardModeV1,
        #[serde(default)]
        offset_along_direction: f32,
        #[serde(default)]
        offset_along_normal: f32,
        scale: [f32; 3],
        #[serde(default)]
        growth_per_second: [f32; 3],
        color: [f32; 4],
        lifetime_seconds: f32,
        #[serde(default)]
        fade_start_fraction: f32,
        #[serde(default)]
        fade_in_fraction: f32,
        #[serde(default)]
        drag_per_second: f32,
        #[serde(default)]
        rotation_radians: f32,
        #[serde(default)]
        rotation_random_radians: f32,
        #[serde(default)]
        spin_radians_per_second: f32,
        #[serde(default)]
        light: Option<FxdLightV1>,
    },
    Tracer {
        primitive: String,
        color: [f32; 4],
        half_length: f32,
        radius: f32,
        speed: f32,
        max_lifetime_seconds: f32,
    },
    Burst {
        kind: FxdLayerKindV1,
        primitive: String,
        #[serde(default)]
        role: FxdRenderRoleV1,
        #[serde(default)]
        texture: String,
        #[serde(default)]
        billboard: FxdBillboardModeV1,
        count: u16,
        scale: [f32; 3],
        color: [f32; 4],
        speed_min: f32,
        speed_max: f32,
        #[serde(default = "default_burst_cone_angle_degrees")]
        cone_angle_degrees: f32,
        #[serde(default)]
        size_variance: f32,
        #[serde(default)]
        lifetime_variance: f32,
        #[serde(default)]
        acceleration: [f32; 3],
        #[serde(default)]
        drag_per_second: f32,
        #[serde(default)]
        rotation_random_radians: f32,
        #[serde(default)]
        spin_radians_per_second: f32,
        #[serde(default)]
        spin_variance: f32,
        lifetime_seconds: f32,
        #[serde(default)]
        fade_start_fraction: f32,
        #[serde(default)]
        fade_in_fraction: f32,
    },
    Decal {
        primitive: String,
        scale: [f32; 3],
        color: [f32; 4],
        #[serde(default)]
        normal_offset: f32,
        lifetime_seconds: f32,
        #[serde(default)]
        fade_start_fraction: f32,
    },
}

fn default_burst_cone_angle_degrees() -> f32 {
    75.0
}

impl FxdLayerV1 {
    fn validate(&self, texture_ids: &BTreeSet<String>) -> Result<(), String> {
        fn finite(values: &[f32]) -> bool {
            values.iter().all(|value| value.is_finite())
        }
        fn validate_texture(texture: &str, ids: &BTreeSet<String>) -> Result<(), String> {
            let texture = texture.trim();
            if texture.is_empty() {
                return Ok(());
            }
            let key = texture.to_ascii_lowercase();
            if !ids.contains(&key) {
                return Err(format!("references unknown texture '{texture}'"));
            }
            Ok(())
        }
        match self {
            Self::Pulse {
                primitive,
                texture,
                offset_along_direction,
                offset_along_normal,
                scale,
                growth_per_second,
                color,
                lifetime_seconds,
                fade_start_fraction,
                fade_in_fraction,
                drag_per_second,
                rotation_radians,
                rotation_random_radians,
                spin_radians_per_second,
                light,
                ..
            } => {
                validate_primitive(primitive)?;
                validate_texture(texture, texture_ids)?;
                if !finite(&[
                    *offset_along_direction,
                    *offset_along_normal,
                    *lifetime_seconds,
                    *fade_start_fraction,
                    *fade_in_fraction,
                    *drag_per_second,
                    *rotation_radians,
                    *rotation_random_radians,
                    *spin_radians_per_second,
                ]) || !finite(scale)
                    || !finite(growth_per_second)
                    || !finite(color)
                    || *lifetime_seconds <= 0.0
                    || *drag_per_second < 0.0
                    || *rotation_random_radians < 0.0
                    || !(0.0..=1.0).contains(fade_in_fraction)
                {
                    return Err("pulse contains invalid numeric data".to_owned());
                }
                if let Some(light) = light {
                    if !finite(&light.color)
                        || !light.intensity.is_finite()
                        || !light.range.is_finite()
                        || light.intensity < 0.0
                        || light.range < 0.0
                    {
                        return Err("pulse light contains invalid numeric data".to_owned());
                    }
                }
            }
            Self::Tracer {
                primitive,
                color,
                half_length,
                radius,
                speed,
                max_lifetime_seconds,
            } => {
                validate_primitive(primitive)?;
                if !finite(color)
                    || !finite(&[*half_length, *radius, *speed, *max_lifetime_seconds])
                    || *half_length <= 0.0
                    || *radius <= 0.0
                    || *speed <= 0.0
                    || *max_lifetime_seconds <= 0.0
                {
                    return Err("tracer contains invalid numeric data".to_owned());
                }
            }
            Self::Burst {
                primitive,
                texture,
                count,
                scale,
                color,
                speed_min,
                speed_max,
                cone_angle_degrees,
                size_variance,
                lifetime_variance,
                acceleration,
                drag_per_second,
                rotation_random_radians,
                spin_radians_per_second,
                spin_variance,
                lifetime_seconds,
                fade_start_fraction,
                fade_in_fraction,
                ..
            } => {
                validate_primitive(primitive)?;
                validate_texture(texture, texture_ids)?;
                if *count == 0
                    || !finite(scale)
                    || !finite(color)
                    || !finite(acceleration)
                    || !finite(&[
                        *speed_min,
                        *speed_max,
                        *cone_angle_degrees,
                        *size_variance,
                        *lifetime_variance,
                        *drag_per_second,
                        *rotation_random_radians,
                        *spin_radians_per_second,
                        *spin_variance,
                        *lifetime_seconds,
                        *fade_start_fraction,
                        *fade_in_fraction,
                    ])
                    || *speed_min < 0.0
                    || *speed_max < *speed_min
                    || !(0.0..=180.0).contains(cone_angle_degrees)
                    || !(0.0..=1.0).contains(size_variance)
                    || !(0.0..=1.0).contains(lifetime_variance)
                    || *drag_per_second < 0.0
                    || *rotation_random_radians < 0.0
                    || *spin_variance < 0.0
                    || !(0.0..=1.0).contains(fade_in_fraction)
                    || *lifetime_seconds <= 0.0
                {
                    return Err("burst contains invalid numeric data".to_owned());
                }
            }
            Self::Decal {
                primitive,
                scale,
                color,
                normal_offset,
                lifetime_seconds,
                fade_start_fraction,
            } => {
                validate_primitive(primitive)?;
                if !finite(scale)
                    || !finite(color)
                    || !finite(&[*normal_offset, *lifetime_seconds, *fade_start_fraction])
                    || *lifetime_seconds <= 0.0
                {
                    return Err("decal contains invalid numeric data".to_owned());
                }
            }
        }
        Ok(())
    }
}

fn validate_primitive(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 {
        Err("primitive must contain 1..=128 bytes".to_owned())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dictionary_rejects_unknown_layer_texture() {
        let dictionary = FxdDictionaryV1 {
            effects: vec![FxdEffectV1 {
                id: "weapon.shot".to_owned(),
                layers: vec![FxdLayerV1::Burst {
                    kind: FxdLayerKindV1::Spark,
                    primitive: "cube".to_owned(),
                    role: FxdRenderRoleV1::Transparent,
                    texture: "missing".to_owned(),
                    billboard: FxdBillboardModeV1::VelocityAligned,
                    count: 4,
                    scale: [0.01, 0.01, 0.05],
                    color: [1.0; 4],
                    speed_min: 1.0,
                    speed_max: 2.0,
                    cone_angle_degrees: 65.0,
                    size_variance: 0.25,
                    lifetime_variance: 0.20,
                    acceleration: [0.0, -9.81, 0.0],
                    drag_per_second: 0.1,
                    rotation_random_radians: 3.14159,
                    spin_radians_per_second: 2.0,
                    spin_variance: 1.0,
                    lifetime_seconds: 0.2,
                    fade_start_fraction: 0.5,
                    fade_in_fraction: 0.0,
                }],
                ..FxdEffectV1::default()
            }],
            ..FxdDictionaryV1::default()
        };
        assert!(dictionary.validate().is_err());
    }
}
