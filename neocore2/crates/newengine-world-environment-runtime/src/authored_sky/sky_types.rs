use super::*;

// Sky lifecycle applies resolved world-environment frames to the scene.
// engine.world.environment owns atmospheric meaning, celestial math, weather and clouds;
// this file only keeps the legacy dome/light bridge alive while render packets mature.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkyVisualKind {
    Dome,
}

#[derive(Clone, Debug)]
pub struct SkyVisualRuntime {
    pub kind: SkyVisualKind,
}

#[derive(Clone, Debug)]
pub struct GameReadyEnvironmentVisualAssetsRuntime {
    pub visual_assets: newengine_world_environment_api::EnvironmentVisualAssetRefsDto,
}

#[derive(Clone, Debug)]
pub struct SkyAtmosphereRuntime {
    pub radius: f32,
    pub profile: GameReadySkyAtmosphereSpec,
}

#[derive(Clone, Copy, Debug)]
pub struct CloudSunOcclusionRuntime {
    pub raw_density: f32,
    pub smoothed_density: f32,
    pub optical_depth: f32,
    pub transmittance: f32,
    pub direct_light_scale: f32,
    pub world_shadow_strength: f32,
}

impl Default for CloudSunOcclusionRuntime {
    fn default() -> Self {
        Self {
            raw_density: 0.0,
            smoothed_density: 0.0,
            optical_depth: 0.0,
            transmittance: 1.0,
            direct_light_scale: 1.0,
            world_shadow_strength: 0.0,
        }
    }
}

pub const SKY_VISUAL_SPAWN_ORDER: [SkyVisualKind; 1] = [
    // The procedural sky shader already renders sun and moon from engine.time / sky-cycle data.
    // Do not spawn extra follow-camera disc meshes: they behave like frame-attached
    // sprites and can be mistaken for incomplete loading or UI leakage.
    SkyVisualKind::Dome,
];

impl SkyVisualKind {
    #[inline]
    pub fn entity_name(self) -> &'static str {
        match self {
            SkyVisualKind::Dome => "Sky/Imported-SkyDome",
        }
    }

    #[inline]
    pub fn initial_color(self, dome_color: [f32; 4]) -> [f32; 4] {
        match self {
            SkyVisualKind::Dome => dome_color,
        }
    }

    #[inline]
    pub fn initial_radius(self, spec: &GameReadySkySpec) -> f32 {
        match self {
            SkyVisualKind::Dome => spec.radius,
        }
    }

    #[inline]
    pub fn primitive_id(self, dome_primitive_id: PrimitiveId) -> PrimitiveId {
        match self {
            SkyVisualKind::Dome => dome_primitive_id,
        }
    }
}

#[inline]
pub fn sky_atmosphere_from_spec(spec: &GameReadySkySpec) -> SkyAtmosphereRuntime {
    SkyAtmosphereRuntime {
        radius: spec.radius,
        profile: spec.atmosphere.clone(),
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn attach_sky_visual_runtime(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    entity: EntityId,
    material_id: MaterialId,
    kind: SkyVisualKind,
    color: [f32; 4],
    definition_ref: Option<String>,
    asset_ref: Option<String>,
    render_options: MeshRenderOptions,
) {
    let _ = world.remove::<Bounds>(entity);
    let _ = world.insert(
        entity,
        EnvironmentDomeRenderState {
            definition_ref,
            asset_ref,
            uv_transform: [1.0, 1.0, 0.0, 0.0],
            material_params: [0.42, 0.72, 0.10, 0.18],
            emissive_params: [2.0, 0.72, 0.0],
        },
    );
    let _ = world.insert(entity, SkyVisualRuntime { kind });
    let _ = world.insert(entity, render_options);
    let _ = apply_exact_material(world, mats, entity, material_id, material_id, color);
}

#[derive(Clone, Copy, Debug)]
pub struct SkyFrameSample {
    pub(crate) to_sun: Vec3,
    pub(crate) sky_tint: [f32; 4],
    pub(crate) cloud_tint: [f32; 4],
    pub(crate) sun_color: [f32; 3],
    pub(crate) sun_intensity: f32,
    pub(crate) ambient_color: [f32; 3],
    pub(crate) ambient_intensity: f32,
    pub(crate) cloud_coverage: f32,
    pub(crate) cloud_softness: f32,
    pub(crate) cloud_shadow_strength: f32,
    pub(crate) haze_amount: f32,
    pub(crate) cloud_advection: Vec2,
    /// Stable environment-owned seed used to reconstruct the procedural cloud
    /// field at runtime startup instead of resetting every session to phase zero.
    pub(crate) cloud_field_seed: u64,
    /// Absolute environment/world time used only to establish the initial cloud
    /// phase. Subsequent frames integrate dt so weather changes remain continuous.
    pub(crate) cloud_world_time_seconds: f64,
    pub(crate) rayleigh_strength: f32,
    pub(crate) mie_strength: f32,
    pub(crate) star_intensity: f32,
    pub(crate) cloud_gust_strength: f32,
    pub(crate) cloud_overcast: f32,
    pub(crate) cloud_light_absorption: f32,
    /// Resolved low cloud deck base and geometric thickness in meters.
    pub(crate) cloud_base_altitude_m: f32,
    pub(crate) cloud_thickness_m: f32,
    /// Low/deep cloud optical morphology plus the independent high deck.
    pub(crate) cloud_layer_density: f32,
    pub(crate) high_cloud_coverage: f32,
    pub(crate) high_cloud_density: f32,
    /// Atmospheric state used by droplet scattering and aerosol extinction.
    pub(crate) humidity: f32,
    pub(crate) aerosol_density: f32,
    pub(crate) precipitation_intensity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SkyDynamicsRuntime {
    pub initialized: bool,
    pub cloud_offset: Vec2,
    pub smoothed_wind: Vec2,
    pub smoothed_coverage: f32,
    pub smoothed_softness: f32,
    pub smoothed_shadow: f32,
    pub smoothed_haze: f32,
    pub smoothed_cloud_base_altitude_m: f32,
    pub smoothed_cloud_thickness_m: f32,
    pub smoothed_cloud_layer_density: f32,
    pub smoothed_high_cloud_coverage: f32,
    pub smoothed_high_cloud_density: f32,
    pub smoothed_humidity: f32,
    pub smoothed_aerosol_density: f32,
    pub smoothed_precipitation_intensity: f32,
    pub evolution_phase: f32,
    pub lifecycle_phase: f32,
    pub gust_phase: f32,
    pub smoothed_sun_occlusion: f32,
}

impl Default for SkyDynamicsRuntime {
    fn default() -> Self {
        Self {
            initialized: false,
            cloud_offset: Vec2::ZERO,
            smoothed_wind: Vec2::ZERO,
            smoothed_coverage: 0.35,
            smoothed_softness: 0.70,
            smoothed_shadow: 0.12,
            smoothed_haze: 0.10,
            smoothed_cloud_base_altitude_m: 1250.0,
            smoothed_cloud_thickness_m: 1100.0,
            smoothed_cloud_layer_density: 0.16,
            smoothed_high_cloud_coverage: 0.08,
            smoothed_high_cloud_density: 0.04,
            smoothed_humidity: 0.45,
            smoothed_aerosol_density: 0.12,
            smoothed_precipitation_intensity: 0.0,
            evolution_phase: 0.0,
            lifecycle_phase: 0.31,
            gust_phase: 0.0,
            smoothed_sun_occlusion: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SkyDynamicsFrame {
    pub cloud_offset: Vec2,
    pub coverage: f32,
    pub softness: f32,
    pub shadow_strength: f32,
    pub haze: f32,
    pub evolution_phase: f32,
    pub lifecycle: f32,
    pub gust_factor: f32,
    pub previous_cloud_offset: Vec2,
    pub previous_evolution_phase: f32,
    pub previous_lifecycle: f32,
    pub temporal_history_weight: f32,
    pub sun_occlusion: CloudSunOcclusionRuntime,
}

#[derive(Clone, Debug)]
pub struct SkyCycleRuntime {
    /// Project-authored stable identity for the active world/scene. Runtime code must never
    /// synthesize a product-specific world id.
    pub world_instance_id: String,
    pub anchor: Option<EntityId>,
    pub sun: Option<EntityId>,
    pub enabled: bool,
    pub time_of_day_hours: f32,
    pub day_length_seconds: f32,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
    /// Authoritative environment-provider routing for this world.
    pub environment_profile: String,
    pub environment_region: String,
    pub environment_biome: String,
    /// Presentation policy such as `cloudless`; it may intentionally suppress
    /// cloud visuals while retaining the physical atmosphere/wind provider.
    pub cloud_profile: String,
    pub base_sun_color: [f32; 3],
    pub base_sun_intensity: f32,
    pub base_ambient_color: [f32; 3],
    pub base_ambient_intensity: f32,
    pub day_index: u64,
}
