use super::*;

// Sky lifecycle applies resolved world-environment frames to the scene.
// engine.world.environment owns atmospheric meaning, celestial math, weather and clouds;
// this file only keeps the legacy dome/light bridge alive while render packets mature.

#[derive(Clone, Debug)]
pub(crate) struct SkyDomeRuntime {
    pub definition_ref: Option<String>,
    pub asset_ref: Option<String>,
    /// Per-instance sky parameters packed into the existing lit-instance ABI.
    /// xy/zw are consumed as UV scale/offset; material params carry cloud and
    /// atmospheric controls; emissive params carry scattering/star controls.
    pub uv_transform: [f32; 4],
    pub material_params: [f32; 4],
    pub emissive_params: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkyVisualKind {
    Dome,
}

#[derive(Clone, Debug)]
pub(crate) struct SkyVisualRuntime {
    pub kind: SkyVisualKind,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SkyClearColorRuntime {
    pub color: [f32; 4],
}

#[derive(Clone, Debug)]
pub(crate) struct GameReadyEnvironmentVisualAssetsRuntime {
    pub visual_assets: newengine_world_environment_api::EnvironmentVisualAssetRefsDto,
}

#[derive(Clone, Debug)]
pub(crate) struct SkyAtmosphereRuntime {
    pub radius: f32,
    pub profile: GameReadySkyAtmosphereSpec,
}

/// Environment-driven display intent consumed by the renderer's stable post-FX
/// contract. Sky/weather owns the artistic target; the render backend still owns
/// exposure adaptation, bloom, color grading and display encoding execution.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SkyPostFxRuntime {
    pub exposure: f32,
    pub gamma: f32,
    pub black_lift: f32,
    pub saturation: f32,
    pub contrast: f32,
    pub temperature: f32,
    pub vignette_strength: f32,
    pub local_contrast_strength: f32,
    pub dither_strength: f32,
    pub bloom_threshold: f32,
    pub bloom_knee: f32,
    pub bloom_intensity: f32,
    pub bloom_radius: f32,
    pub sun_glare_scale: f32,
    pub sun_ray_scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CloudSunOcclusionRuntime {
    pub raw_density: f32,
    pub smoothed_density: f32,
    pub optical_depth: f32,
    pub transmittance: f32,
    pub direct_light_scale: f32,
    pub world_shadow_strength: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialCloudShadowRuntime {
    /// xy: integrated wind offset, z: morphology phase, w: lifecycle.
    pub map0: [f32; 4],
    /// x: world frequency, y: cloud altitude, z: coverage, w: edge softness.
    pub map1: [f32; 4],
    /// x: local shadow strength, y: absorption, z: broad direct-light scale,
    /// w: enabled flag.
    pub map2: [f32; 4],
    /// Previous cloud transform: offset.xy, evolution phase, lifecycle.
    /// This is an analytic temporal-reprojection history in cloud-domain space.
    pub map3: [f32; 4],
    /// x: history weight, y: erosion frequency, z: erosion strength,
    /// w: near-detail fade distance in world units.
    pub map4: [f32; 4],
    pub broad_ambient_scale: f32,
}

impl Default for SpatialCloudShadowRuntime {
    fn default() -> Self {
        Self {
            map0: [0.0, 0.0, 0.0, 0.5],
            map1: [0.0042, 1800.0, 0.0, 0.70],
            map2: [0.0, 0.0, 1.0, 0.0],
            map3: [0.0, 0.0, 0.0, 0.5],
            map4: [0.0, 0.032, 0.14, 96.0],
            broad_ambient_scale: 1.0,
        }
    }
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

impl Default for SkyPostFxRuntime {
    fn default() -> Self {
        Self {
            exposure: 1.12,
            gamma: 2.2,
            black_lift: 0.0,
            saturation: 1.04,
            contrast: 1.02,
            temperature: 0.0,
            vignette_strength: 0.055,
            local_contrast_strength: 0.060,
            dither_strength: 1.0,
            bloom_threshold: 1.05,
            bloom_knee: 0.30,
            bloom_intensity: 0.060,
            bloom_radius: 1.0,
            sun_glare_scale: 1.0,
            sun_ray_scale: 1.0,
        }
    }
}

pub(in crate::scene_bridge::game_ready) const SKY_VISUAL_SPAWN_ORDER: [SkyVisualKind; 1] = [
    // The procedural sky shader already renders sun and moon from engine.time / sky-cycle data.
    // Do not spawn extra follow-camera disc meshes: they behave like frame-attached
    // sprites and can be mistaken for incomplete loading or UI leakage.
    SkyVisualKind::Dome,
];

impl SkyVisualKind {
    #[inline]
    pub(in crate::scene_bridge::game_ready) fn entity_name(self) -> &'static str {
        match self {
            SkyVisualKind::Dome => "Sky/Imported-SkyDome",
        }
    }

    #[inline]
    pub(in crate::scene_bridge::game_ready) fn initial_color(
        self,
        dome_color: [f32; 4],
    ) -> [f32; 4] {
        match self {
            SkyVisualKind::Dome => dome_color,
        }
    }

    #[inline]
    pub(in crate::scene_bridge::game_ready) fn initial_radius(
        self,
        spec: &GameReadySkySpec,
    ) -> f32 {
        match self {
            SkyVisualKind::Dome => spec.radius,
        }
    }

    #[inline]
    pub(in crate::scene_bridge::game_ready) fn primitive_id(
        self,
        dome_primitive_id: PrimitiveId,
    ) -> PrimitiveId {
        match self {
            SkyVisualKind::Dome => dome_primitive_id,
        }
    }
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_atmosphere_from_spec(
    spec: &GameReadySkySpec,
) -> SkyAtmosphereRuntime {
    SkyAtmosphereRuntime {
        radius: spec.radius,
        profile: spec.atmosphere.clone(),
    }
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn attach_sky_visual_runtime(
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
        SkyDomeRuntime {
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
pub(in crate::scene_bridge::game_ready) struct SkyFrameSample {
    pub(in crate::scene_bridge::game_ready) to_sun: Vec3,
    pub(in crate::scene_bridge::game_ready) sky_tint: [f32; 4],
    pub(in crate::scene_bridge::game_ready) cloud_tint: [f32; 4],
    pub(in crate::scene_bridge::game_ready) sun_color: [f32; 3],
    pub(in crate::scene_bridge::game_ready) sun_intensity: f32,
    pub(in crate::scene_bridge::game_ready) ambient_color: [f32; 3],
    pub(in crate::scene_bridge::game_ready) ambient_intensity: f32,
    pub(in crate::scene_bridge::game_ready) cloud_coverage: f32,
    pub(in crate::scene_bridge::game_ready) cloud_softness: f32,
    pub(in crate::scene_bridge::game_ready) cloud_shadow_strength: f32,
    pub(in crate::scene_bridge::game_ready) haze_amount: f32,
    pub(in crate::scene_bridge::game_ready) cloud_advection: Vec2,
    pub(in crate::scene_bridge::game_ready) rayleigh_strength: f32,
    pub(in crate::scene_bridge::game_ready) mie_strength: f32,
    pub(in crate::scene_bridge::game_ready) star_intensity: f32,
    pub(in crate::scene_bridge::game_ready) cloud_gust_strength: f32,
    pub(in crate::scene_bridge::game_ready) cloud_overcast: f32,
    pub(in crate::scene_bridge::game_ready) cloud_light_absorption: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SkyDynamicsRuntime {
    pub initialized: bool,
    pub cloud_offset: Vec2,
    pub smoothed_wind: Vec2,
    pub smoothed_coverage: f32,
    pub smoothed_softness: f32,
    pub smoothed_shadow: f32,
    pub smoothed_haze: f32,
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
            evolution_phase: 0.0,
            lifecycle_phase: 0.31,
            gust_phase: 0.0,
            smoothed_sun_occlusion: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::scene_bridge::game_ready) struct SkyDynamicsFrame {
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
pub(crate) struct SkyCycleRuntime {
    pub anchor: Option<EntityId>,
    pub sun: Option<EntityId>,
    pub enabled: bool,
    pub time_of_day_hours: f32,
    pub day_length_seconds: f32,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
    pub base_sun_color: [f32; 3],
    pub base_sun_intensity: f32,
    pub base_ambient_color: [f32; 3],
    pub base_ambient_intensity: f32,
    pub day_index: u64,
}
