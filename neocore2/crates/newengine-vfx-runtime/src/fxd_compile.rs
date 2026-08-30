use std::collections::BTreeMap;

use newengine_math::Vec3;
use newengine_primitives::{builtins, PrimitiveId};
use newengine_vfx_api::{
    FxdAlignmentV1, FxdBillboardModeV1, FxdDictionaryV1, FxdLayerKindV1, FxdLayerV1,
    FxdRenderRoleV1, VfxEffectRef, VfxGpuBillboardMode, VfxGpuTextureRegistry,
};

use crate::{
    VfxAlignment, VfxEffectDefinition, VfxEffectLibrary, VfxLayerDefinition, VfxLayerKind,
    VfxLightDefinition, VfxRenderRole,
};

impl VfxEffectLibrary {
    /// Registers a complete project-owned FXD dictionary.
    ///
    /// `logical_path` is part of effect identity: an authored effect `shot` becomes
    /// `effects/weapons/rifle.fxd@shot`. Runtime code never invents weapon effect ids.
    pub fn register_fxd_dictionary(
        &mut self,
        dictionary: &FxdDictionaryV1,
        logical_path: &str,
        textures: &mut VfxGpuTextureRegistry,
    ) -> Result<Vec<String>, String> {
        dictionary.validate()?;
        let logical_path = canonical_dictionary_path(logical_path)?;
        let mut texture_slots = BTreeMap::<String, u8>::new();
        for texture in &dictionary.textures {
            let slot = textures.register(&texture.source)?;
            texture_slots.insert(texture.id.trim().to_ascii_lowercase(), slot);
        }

        let mut registered = Vec::with_capacity(dictionary.effects.len());
        for authored in &dictionary.effects {
            let effect_ref = format!("{}@{}", logical_path, authored.id.trim());
            let layers = authored
                .layers
                .iter()
                .map(|layer| compile_layer(layer, &texture_slots))
                .collect::<Result<Vec<_>, _>>()?;
            self.register(VfxEffectDefinition {
                effect: VfxEffectRef::new(effect_ref.clone()),
                priority: authored.priority,
                layers,
            })?;
            registered.push(effect_ref);
        }
        Ok(registered)
    }
}

fn canonical_dictionary_path(raw: &str) -> Result<String, String> {
    let path = raw.trim().replace('\\', "/");
    if path.is_empty() || !path.to_ascii_lowercase().ends_with(".fxd") || path.contains('@') {
        return Err(format!(
            "FXD logical path must be a selector-free .fxd path, got='{raw}'"
        ));
    }
    Ok(path)
}

fn compile_layer(
    layer: &FxdLayerV1,
    textures: &BTreeMap<String, u8>,
) -> Result<VfxLayerDefinition, String> {
    Ok(match layer {
        FxdLayerV1::Pulse {
            kind,
            primitive,
            role,
            alignment,
            texture,
            billboard,
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
        } => VfxLayerDefinition::Pulse {
            kind: layer_kind(*kind),
            primitive: primitive_id(primitive)?,
            role: render_role(*role),
            alignment: alignment_mode(*alignment),
            texture_slot: texture_slot(texture, textures)?,
            billboard: billboard_mode(*billboard),
            offset_along_direction: *offset_along_direction,
            offset_along_normal: *offset_along_normal,
            scale: v3(*scale),
            growth_per_second: v3(*growth_per_second),
            color: *color,
            lifetime_seconds: *lifetime_seconds,
            fade_start_fraction: *fade_start_fraction,
            fade_in_fraction: *fade_in_fraction,
            drag_per_second: *drag_per_second,
            rotation_radians: *rotation_radians,
            rotation_random_radians: *rotation_random_radians,
            spin_radians_per_second: *spin_radians_per_second,
            light: light.map(|light| VfxLightDefinition {
                color: light.color,
                intensity: light.intensity,
                range: light.range,
            }),
        },
        FxdLayerV1::Tracer {
            primitive,
            color,
            half_length,
            radius,
            speed,
            max_lifetime_seconds,
        } => VfxLayerDefinition::Tracer {
            primitive: primitive_id(primitive)?,
            color: *color,
            half_length: *half_length,
            radius: *radius,
            speed: *speed,
            max_lifetime_seconds: *max_lifetime_seconds,
        },
        FxdLayerV1::Burst {
            kind,
            primitive,
            role,
            texture,
            billboard,
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
        } => VfxLayerDefinition::Burst {
            kind: layer_kind(*kind),
            primitive: primitive_id(primitive)?,
            role: render_role(*role),
            texture_slot: texture_slot(texture, textures)?,
            billboard: billboard_mode(*billboard),
            count: *count,
            scale: v3(*scale),
            color: *color,
            speed_min: *speed_min,
            speed_max: *speed_max,
            cone_angle_degrees: *cone_angle_degrees,
            size_variance: *size_variance,
            lifetime_variance: *lifetime_variance,
            acceleration: v3(*acceleration),
            drag_per_second: *drag_per_second,
            rotation_random_radians: *rotation_random_radians,
            spin_radians_per_second: *spin_radians_per_second,
            spin_variance: *spin_variance,
            lifetime_seconds: *lifetime_seconds,
            fade_start_fraction: *fade_start_fraction,
            fade_in_fraction: *fade_in_fraction,
        },
        FxdLayerV1::Decal {
            primitive,
            scale,
            color,
            normal_offset,
            lifetime_seconds,
            fade_start_fraction,
        } => VfxLayerDefinition::Decal {
            primitive: primitive_id(primitive)?,
            scale: v3(*scale),
            color: *color,
            normal_offset: *normal_offset,
            lifetime_seconds: *lifetime_seconds,
            fade_start_fraction: *fade_start_fraction,
        },
    })
}

fn texture_slot(texture: &str, textures: &BTreeMap<String, u8>) -> Result<u8, String> {
    let texture = texture.trim();
    if texture.is_empty() {
        return Ok(0);
    }
    textures
        .get(&texture.to_ascii_lowercase())
        .copied()
        .ok_or_else(|| format!("FXD layer references unregistered texture '{texture}'"))
}

fn primitive_id(value: &str) -> Result<PrimitiveId, String> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "cube" => Ok(builtins::ID_CUBE),
        "plane" => Ok(builtins::ID_PLANE),
        "grid" => Ok(builtins::ID_GRID),
        "sphere" | "sphere_uv" => Ok(builtins::ID_SPHERE_UV),
        "cylinder" => Ok(builtins::ID_CYLINDER),
        "cone" => Ok(builtins::ID_CONE),
        "capsule" => Ok(builtins::ID_CAPSULE),
        "torus" => Ok(builtins::ID_TORUS),
        "disc" => Ok(builtins::ID_DISC),
        other => Err(format!("unsupported FXD primitive '{other}'")),
    }
}

#[inline]
fn v3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[inline]
fn render_role(value: FxdRenderRoleV1) -> VfxRenderRole {
    match value {
        FxdRenderRoleV1::Transparent => VfxRenderRole::Transparent,
        FxdRenderRoleV1::Decal => VfxRenderRole::Decal,
    }
}

#[inline]
fn alignment_mode(value: FxdAlignmentV1) -> VfxAlignment {
    match value {
        FxdAlignmentV1::None => VfxAlignment::None,
        FxdAlignmentV1::DirectionY => VfxAlignment::DirectionY,
        FxdAlignmentV1::DirectionZ => VfxAlignment::DirectionZ,
        FxdAlignmentV1::NormalY => VfxAlignment::NormalY,
    }
}

#[inline]
fn billboard_mode(value: FxdBillboardModeV1) -> VfxGpuBillboardMode {
    match value {
        FxdBillboardModeV1::CameraFacing => VfxGpuBillboardMode::CameraFacing,
        FxdBillboardModeV1::VelocityAligned => VfxGpuBillboardMode::VelocityAligned,
    }
}

#[inline]
fn layer_kind(value: FxdLayerKindV1) -> VfxLayerKind {
    match value {
        FxdLayerKindV1::MuzzleFlash => VfxLayerKind::MuzzleFlash,
        FxdLayerKindV1::MuzzleCore => VfxLayerKind::MuzzleCore,
        FxdLayerKindV1::Smoke => VfxLayerKind::Smoke,
        FxdLayerKindV1::Tracer => VfxLayerKind::Tracer,
        FxdLayerKindV1::Spark => VfxLayerKind::Spark,
        FxdLayerKindV1::ImpactDecal => VfxLayerKind::ImpactDecal,
        FxdLayerKindV1::Trail => VfxLayerKind::Trail,
        FxdLayerKindV1::Debris => VfxLayerKind::Debris,
        FxdLayerKindV1::Generic => VfxLayerKind::Generic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_vfx_api::{FxdEffectV1, FxdTextureV1};

    #[test]
    fn project_effect_identity_is_dictionary_scoped() {
        let dictionary = FxdDictionaryV1 {
            textures: vec![FxdTextureV1 {
                id: "spark".to_owned(),
                source: "textures/vfx/weapon.ytd@spark".to_owned(),
            }],
            effects: vec![FxdEffectV1 {
                id: "impact.metal".to_owned(),
                layers: vec![FxdLayerV1::Burst {
                    kind: FxdLayerKindV1::Spark,
                    primitive: "cube".to_owned(),
                    role: FxdRenderRoleV1::Transparent,
                    texture: "spark".to_owned(),
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
        let mut library = VfxEffectLibrary::default();
        let mut textures = VfxGpuTextureRegistry::default();
        let registered = library
            .register_fxd_dictionary(&dictionary, "effects/weapons/rifle.fxd", &mut textures)
            .unwrap();
        assert_eq!(registered, vec!["effects/weapons/rifle.fxd@impact.metal"]);
        assert!(library.get(&registered[0]).is_some());
        assert_eq!(textures.slot_path(1), Some("textures/vfx/weapon.ytd@spark"));
    }
}
