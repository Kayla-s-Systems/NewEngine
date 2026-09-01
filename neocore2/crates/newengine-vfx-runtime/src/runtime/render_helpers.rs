fn gpu_particle_kind(kind: VfxLayerKind) -> Option<VfxGpuParticleKind> {
    match kind {
        VfxLayerKind::Smoke => Some(VfxGpuParticleKind::Smoke),
        VfxLayerKind::Spark => Some(VfxGpuParticleKind::Spark),
        VfxLayerKind::Debris => Some(VfxGpuParticleKind::Debris),
        VfxLayerKind::MuzzleFlash => Some(VfxGpuParticleKind::MuzzleFlash),
        VfxLayerKind::MuzzleCore => Some(VfxGpuParticleKind::MuzzleCore),
        _ => None,
    }
}

fn render_options(role: VfxRenderRole) -> MeshRenderOptions {
    let mut options = MeshRenderOptions::world_opaque();
    options.role = match role {
        VfxRenderRole::Transparent => MeshRenderRole::WorldTransparent,
        VfxRenderRole::Decal => MeshRenderRole::Decal,
    };
    options.depth_policy = MeshDepthPolicy::ReadOnly;
    options.shadow_policy = MeshShadowPolicy::None;
    options.cull_policy = MeshCullPolicy::None;
    options.sort_policy = MeshSortPolicy::Transparent;
    options
}

pub(crate) fn resolve_emission_axis(mode: VfxEmissionAxis, direction: Vec3, normal: Vec3) -> Vec3 {
    let direction = direction.normalize_or_zero();
    let normal = normal.normalize_or_zero();
    let fallback_normal = if normal.length_squared() > 1.0e-8 {
        normal
    } else {
        Vec3::Y
    };
    match mode {
        VfxEmissionAxis::Normal => fallback_normal,
        VfxEmissionAxis::Direction => {
            if direction.length_squared() > 1.0e-8 {
                direction
            } else {
                fallback_normal
            }
        }
        VfxEmissionAxis::Reflection => {
            if direction.length_squared() <= 1.0e-8 || normal.length_squared() <= 1.0e-8 {
                fallback_normal
            } else {
                let reflected = direction - normal * (2.0 * direction.dot(normal));
                if reflected.length_squared() > 1.0e-8 {
                    reflected.normalize_or_zero()
                } else {
                    fallback_normal
                }
            }
        }
    }
}

fn alignment_rotation(alignment: VfxAlignment, direction: Vec3, normal: Vec3) -> Quat {
    match alignment {
        VfxAlignment::None => Quat::IDENTITY,
        VfxAlignment::DirectionY => {
            Quat::from_rotation_arc(Vec3::Y, direction).normalize_or_identity()
        }
        VfxAlignment::DirectionZ => {
            Quat::from_rotation_arc(Vec3::Z, direction).normalize_or_identity()
        }
        VfxAlignment::NormalY => Quat::from_rotation_arc(Vec3::Y, normal).normalize_or_identity(),
    }
}

fn install_light(
    world: &mut World,
    entity: EntityId,
    definition: VfxLightDefinition,
    request_intensity: f32,
) -> f32 {
    let intensity = definition.intensity * request_intensity;
    let _ = world.insert(
        entity,
        PointLight {
            color: definition.color,
            intensity,
            range: definition.range,
        },
    );
    intensity
}

fn surface_color(kind: VfxLayerKind, base: [f32; 4], response: VfxSurfaceResponse) -> [f32; 4] {
    let mut color = base;
    match kind {
        VfxLayerKind::Spark => {
            if let Some(rgb) = response.spark_color {
                color[..3].copy_from_slice(&rgb);
            }
            color[3] *= if response.spark_alpha_scale.is_finite() {
                response.spark_alpha_scale.max(0.0)
            } else {
                1.0
            };
        }
        VfxLayerKind::Smoke => {
            if let Some(rgb) = response.smoke_color {
                color[..3].copy_from_slice(&rgb);
            }
        }
        VfxLayerKind::ImpactDecal => {
            if let Some(rgb) = response.decal_color {
                color[..3].copy_from_slice(&rgb);
            }
        }
        _ => {}
    }
    color
}
