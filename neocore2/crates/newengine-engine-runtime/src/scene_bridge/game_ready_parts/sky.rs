// Sky lifecycle applies resolved world-environment frames to the scene.
// engine.world.environment owns atmospheric meaning, celestial math, weather and clouds;
// this file only keeps the legacy dome/light bridge alive while render packets mature.

#[derive(Clone, Debug)]
pub(crate) struct SkyDomeRuntime {
    pub follow_camera: bool,
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
pub(crate) struct GameReadyEnvironmentFrameRuntime {
    pub frame: newengine_world_environment_api::EnvironmentFrameDto,
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

pub(super) const SKY_VISUAL_SPAWN_ORDER: [SkyVisualKind; 1] = [
    // The procedural sky shader already renders sun and moon from engine.time / sky-cycle data.
    // Do not spawn extra follow-camera disc meshes: they behave like frame-attached
    // sprites and can be mistaken for incomplete loading or UI leakage.
    SkyVisualKind::Dome,
];

impl SkyVisualKind {
    #[inline]
    pub(super) fn entity_name(self) -> &'static str {
        match self {
            SkyVisualKind::Dome => "Sky/Imported-SkyDome",
        }
    }

    #[inline]
    pub(super) fn initial_color(self, dome_color: [f32; 4]) -> [f32; 4] {
        match self {
            SkyVisualKind::Dome => dome_color,
        }
    }

    #[inline]
    pub(super) fn initial_radius(self, spec: &GameReadySkySpec) -> f32 {
        match self {
            SkyVisualKind::Dome => spec.radius,
        }
    }

    #[inline]
    pub(super) fn follows_camera(self, spec: &GameReadySkySpec) -> bool {
        match self {
            SkyVisualKind::Dome => spec.follow_camera,
        }
    }

    #[inline]
    pub(super) fn primitive_id(self, dome_primitive_id: PrimitiveId) -> PrimitiveId {
        match self {
            SkyVisualKind::Dome => dome_primitive_id,
        }
    }
}

#[inline]
pub(super) fn sky_atmosphere_from_spec(spec: &GameReadySkySpec) -> SkyAtmosphereRuntime {
    SkyAtmosphereRuntime {
        radius: spec.radius,
        profile: spec.atmosphere.clone(),
    }
}

#[inline]
pub(super) fn attach_sky_visual_runtime(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    entity: EntityId,
    material_id: MaterialId,
    kind: SkyVisualKind,
    color: [f32; 4],
    follow_camera: bool,
) {
    let _ = world.remove::<Bounds>(entity);
    let _ = world.insert(entity, SkyDomeRuntime { follow_camera });
    let _ = world.insert(entity, SkyVisualRuntime { kind });
    let _ = apply_exact_material(world, mats, entity, material_id, material_id, color);
}

#[derive(Clone, Copy, Debug)]
struct SkyFrameSample {
    to_sun: Vec3,
    sky_tint: [f32; 4],
    cloud_tint: [f32; 4],
    sun_color: [f32; 3],
    sun_intensity: f32,
    ambient_color: [f32; 3],
    ambient_intensity: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct SkyCycleRuntime {
    pub enabled: bool,
    pub time_of_day_hours: f32,
    pub day_length_seconds: f32,
    pub latitude_degrees: f32,
    pub axial_tilt_degrees: f32,
    pub base_sun_color: [f32; 3],
    pub base_sun_intensity: f32,
    pub base_ambient_color: [f32; 3],
    pub base_ambient_intensity: f32,
}


fn time_snapshot_for_sky_cycle() -> Option<newengine_core::time::TimeSnapshotV1> {
    match call_service_v1_optional(
        newengine_core::time::ENGINE_TIME_SERVICE_ID,
        newengine_core::time::time_method::SNAPSHOT_V1,
        &[],
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<newengine_core::time::TimeSnapshotV1>(&bytes) {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                log::warn!("game-ready sky cycle: engine.time snapshot invalid; keeping authored scene.day_night time for this tick err='{e}'");
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            log::warn!("game-ready sky cycle: engine.time snapshot unavailable; keeping authored scene.day_night time for this tick err='{e}'");
            None
        }
    }
}


fn authored_time_snapshot_for_sky_cycle(cycle: &SkyCycleRuntime) -> newengine_core::time::TimeSnapshotV1 {
    let mut snapshot = newengine_core::time::TimeSnapshotV1::default();
    snapshot.game.seconds_per_game_day = (cycle.day_length_seconds as f64).max(1.0);
    snapshot.game.normalized_day = (cycle.time_of_day_hours as f64 / 24.0).rem_euclid(1.0);
    snapshot.game.seconds_of_day = snapshot.game.normalized_day * snapshot.game.seconds_per_game_day;
    snapshot.game.time_scale = if cycle.enabled { 1.0 } else { 0.0 };
    snapshot
}

fn environment_frame_for_sky_cycle(
    cycle: &SkyCycleRuntime,
    snapshot: newengine_core::time::TimeSnapshotV1,
) -> Option<newengine_world_environment_api::EnvironmentFrameDto> {
    let request = newengine_world_environment_api::EnvironmentFrameRequest {
        frame_id: snapshot.frame_index,
        world_instance_id: "game-ready-fps.world".to_owned(),
        time: snapshot,
        observer_position: newengine_world_environment_api::Vec3Dto::zero(),
        observer_cell: None,
        active_region: Some("game_ready.highlands".to_owned()),
        active_biome: Some("highlands".to_owned()),
        resident_cells: Vec::new(),
        environment_profile: newengine_world_environment_api::EnvironmentProfileRefDto {
            profile_id: "environment.game_ready_highlands".to_owned(),
        },
        seed: 0x4752_4541_4459u64 ^ u64::from(cycle.day_length_seconds.to_bits()),
    };
    let payload = match serde_json::to_vec(&request) {
        Ok(payload) => payload,
        Err(e) => {
            log::warn!("game-ready environment bridge: failed to encode EnvironmentFrameRequest err='{e}'");
            return None;
        }
    };
    match call_service_v1_optional(
        newengine_world_environment_api::ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
        newengine_world_environment_api::WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<newengine_world_environment_api::EnvironmentFrameDto>(&bytes) {
            Ok(frame) => Some(frame),
            Err(e) => {
                log::warn!("game-ready environment bridge: EnvironmentFrameDto decode failed; using explicit degraded authored sky frame err='{e}'");
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            log::warn!("game-ready environment bridge: engine.world.environment unavailable; using explicit degraded authored sky frame err='{e}'");
            None
        }
    }
}


#[inline]
fn sync_game_ready_day_night_to_engine_time(day_night: &GameReadyDayNightSpec) {
    let request = newengine_core::time::TimeGameClockSetRequestV1 {
        day_index: 0,
        seconds_of_day: (day_night.time_of_day_hours as f64 * 3600.0).rem_euclid(86_400.0),
        seconds_per_game_day: (day_night.day_length_seconds as f64).max(1.0),
        time_scale: if day_night.enabled { 1.0 } else { 0.0 },
    };
    let Ok(payload) = serde_json::to_vec(&request) else {
        log::warn!("game-ready sky cycle: failed to encode engine.time clock request");
        return;
    };
    match call_service_v1_optional(
        newengine_core::time::ENGINE_TIME_SERVICE_ID,
        newengine_core::time::time_method::SET_GAME_CLOCK_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<newengine_core::time::TimeSnapshotV1>(&bytes) {
            Ok(snapshot) => log::info!(
                "game-ready sky cycle: engine.time game clock set source='scene.day_night' tod={:.2}h day_len={:.1}s normalized_day={:.6} time_scale={:.3}",
                day_night.time_of_day_hours,
                day_night.day_length_seconds,
                snapshot.game.normalized_day,
                snapshot.game.time_scale
            ),
            Err(e) => log::warn!(
                "game-ready sky cycle: engine.time set_game_clock_v1 returned invalid snapshot err='{}'",
                e
            ),
        },
        Ok(None) => log::debug!(
            "game-ready sky cycle: engine.time route absent; authored scene.day_night time remains fixed until a time provider is active"
        ),
        Err(e) => log::warn!(
            "game-ready sky cycle: engine.time set_game_clock_v1 failed; authored scene.day_night time remains fixed err='{}'",
            e
        ),
    }
}

#[inline]
fn sky_smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-5)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn sky_lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
fn solar_direction_from_cycle(time_hours: f32, latitude_degrees: f32, axial_tilt_degrees: f32) -> Vec3 {
    let latitude = latitude_degrees.to_radians().clamp(-1.5533, 1.5533);
    let axial_tilt = axial_tilt_degrees.to_radians();
    let hour_angle = (time_hours / 24.0) * TAU - core::f32::consts::PI;
    let declination = axial_tilt * (hour_angle + core::f32::consts::FRAC_PI_2).sin();
    let altitude = (latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos()).asin();
    let azimuth = hour_angle + core::f32::consts::PI;
    let horizon = altitude.cos().max(0.0);
    Vec3::new(azimuth.sin() * horizon, altitude.sin(), azimuth.cos() * horizon).normalize_or_zero()
}

#[inline]
fn sky_mul3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
fn sky_clamp3(a: [f32; 3], lo: f32, hi: f32) -> [f32; 3] {
    [a[0].clamp(lo, hi), a[1].clamp(lo, hi), a[2].clamp(lo, hi)]
}

#[inline]
fn sky_color_to_rgba(a: [f32; 3]) -> [f32; 4] {
    [a[0], a[1], a[2], 1.0]
}

#[inline]
fn sky_safe_dir(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.is_finite() && v.length_squared() > 1.0e-6 {
        v.normalize_or_zero()
    } else {
        fallback
    }
}

fn sample_sky_frame(cycle: &SkyCycleRuntime, atmosphere: Option<&SkyAtmosphereRuntime>, to_sun: Vec3) -> SkyFrameSample {
    let to_sun = sky_safe_dir(to_sun, Vec3::new(0.0, 1.0, 0.0));
    let elevation = to_sun.y;

    // A single time-of-day curve drives both light and sky. This prevents the
    // old bug where the terrain was already night-lit but the sky stayed bright.
    let day = sky_smoothstep(-0.08, 0.20, elevation);
    let night = (1.0 - sky_smoothstep(-0.12, 0.18, elevation)).clamp(0.0, 1.0);
    let horizon_glow = 1.0 - sky_smoothstep(0.08, 0.58, elevation.abs());
    let twilight = (horizon_glow * sky_smoothstep(-0.24, 0.10, elevation)).clamp(0.0, 1.0);
    let dusk_mix = (twilight * (1.0 - day * 0.55)).clamp(0.0, 1.0);

    let profile = atmosphere.map(|a| &a.profile);
    let day_zenith = profile.map(|p| p.day_zenith).unwrap_or([0.23, 0.42, 0.82]);
    let day_horizon = profile.map(|p| p.day_horizon).unwrap_or([0.64, 0.78, 0.96]);
    let dusk_zenith = profile.map(|p| p.dusk_zenith).unwrap_or([0.16, 0.20, 0.40]);
    let dusk_horizon = profile.map(|p| p.dusk_horizon).unwrap_or([1.00, 0.47, 0.20]);
    let night_zenith = profile.map(|p| p.night_zenith).unwrap_or([0.006, 0.010, 0.030]);
    let night_horizon = profile.map(|p| p.night_horizon).unwrap_or([0.020, 0.024, 0.052]);
    let cloud_day = profile.map(|p| p.cloud_day).unwrap_or([0.98, 0.96, 0.88]);
    let cloud_night = profile.map(|p| p.cloud_night).unwrap_or([0.040, 0.050, 0.085]);
    let night_sky_strength = profile.map(|p| p.night_sky_strength).unwrap_or(0.35).clamp(0.0, 1.0);
    let cloud_coverage = profile.map(|p| p.cloud_coverage).unwrap_or(0.42).clamp(0.0, 1.0);
    let cloud_softness = profile.map(|p| p.cloud_softness).unwrap_or(0.72).clamp(0.01, 1.0);

    let zenith_base = sky_lerp3(night_zenith, day_zenith, day);
    let horizon_base = sky_lerp3(night_horizon, day_horizon, day);
    let zenith = sky_lerp3(zenith_base, dusk_zenith, dusk_mix);
    let horizon = sky_lerp3(horizon_base, dusk_horizon, dusk_mix);

    let sky_band = (0.33 + 0.34 * twilight).clamp(0.0, 1.0);
    let mut sky_rgb = sky_lerp3(zenith, horizon, sky_band);
    let night_dim = (1.0 - night * (1.0 - night_sky_strength)).clamp(0.03, 1.0);
    sky_rgb = sky_mul3(sky_rgb, night_dim);

    let cloud_visibility = (0.16 + 0.84 * day + 0.22 * twilight + 0.18 * night).clamp(0.0, 1.0);
    let cloud_shape_gain = (1.0 - cloud_coverage * 0.28) * (0.72 + cloud_softness * 0.28);
    let cloud_rgb = sky_mul3(
        sky_lerp3(cloud_night, sky_lerp3(cloud_day, dusk_horizon, twilight * 0.35), day.max(twilight * 0.62)),
        (cloud_visibility * cloud_shape_gain).clamp(0.02, 1.25),
    );

    let warm = [1.0, 0.55, 0.27];
    let moon_light = [0.24, 0.30, 0.48];
    let noon = cycle.base_sun_color;
    let day_color = sky_lerp3(noon, warm, horizon_glow * day);
    let sun_color = sky_lerp3(moon_light, day_color, day.max(twilight * 0.18));
    let sun_intensity = cycle.base_sun_intensity * day.powf(1.18)
        + 0.18 * cycle.base_sun_intensity * twilight
        + 0.025 * night;


    let ambient_color = sky_lerp3(
        sky_lerp3([0.018, 0.024, 0.056], cycle.base_ambient_color, day),
        [0.36, 0.26, 0.20],
        twilight * 0.28,
    );
    let ambient_intensity = cycle.base_ambient_intensity * (0.08 + 0.92 * day)
        + 0.055 * twilight
        + 0.018 * night;

    SkyFrameSample {
        to_sun,
        sky_tint: sky_color_to_rgba(sky_clamp3(sky_rgb, 0.0, 1.0)),
        cloud_tint: sky_color_to_rgba(sky_clamp3(cloud_rgb, 0.0, 1.0)),
        sun_color: sky_clamp3(sun_color, 0.0, 1.0),
        sun_intensity: sun_intensity.max(0.0),
        ambient_color: sky_clamp3(ambient_color, 0.0, 1.0),
        ambient_intensity: ambient_intensity.max(0.0),
    }
}


fn env_vec_to_vec3(v: newengine_world_environment_api::Vec3Dto, fallback: Vec3) -> Vec3 {
    sky_safe_dir(Vec3::new(v.x, v.y, v.z), fallback)
}

#[inline]
fn env_color_to_rgb(c: newengine_world_environment_api::Color3Dto) -> [f32; 3] {
    [c.r.clamp(0.0, 1.0), c.g.clamp(0.0, 1.0), c.b.clamp(0.0, 1.0)]
}

fn sample_sky_frame_from_environment(
    cycle: &SkyCycleRuntime,
    environment: &newengine_world_environment_api::EnvironmentFrameDto,
) -> SkyFrameSample {
    let to_sun = env_vec_to_vec3(environment.celestial.sun.direction_world, Vec3::new(0.0, 1.0, 0.0));
    let render = &environment.consumer_packets.render;
    let day_strength = (render.sun_intensity_hint / 105_000.0).clamp(0.0, 1.0);
    let moon_strength = (render.moon_intensity_hint / 0.25).clamp(0.0, 1.0);
    let overcast_loss = 1.0 - environment.sky.overcast_blend.clamp(0.0, 1.0) * 0.32;
    let sky_rgb = sky_mul3(
        sky_lerp3(
            env_color_to_rgb(environment.sky.zenith_color_linear),
            env_color_to_rgb(environment.sky.horizon_color_linear),
            0.42 + environment.sky.dusk_dawn_blend.clamp(0.0, 1.0) * 0.24,
        ),
        overcast_loss,
    );
    let cloud_rgb = sky_mul3(
        sky_lerp3(
            env_color_to_rgb(environment.sky.horizon_color_linear),
            env_color_to_rgb(environment.sky.sun_horizon_color_linear),
            environment.sky.dusk_dawn_blend.clamp(0.0, 1.0) * 0.45,
        ),
        (0.70 + day_strength * 0.45 - environment.clouds.light_absorption * 0.25).clamp(0.06, 1.20),
    );
    let sun_color = sky_lerp3(
        env_color_to_rgb(environment.celestial.moon.color_linear),
        env_color_to_rgb(environment.celestial.sun.color_linear),
        day_strength.max(environment.sky.dusk_dawn_blend * 0.25),
    );
    let sun_intensity = cycle.base_sun_intensity * day_strength * overcast_loss
        + cycle.base_sun_intensity * 0.025 * moon_strength
        + cycle.base_sun_intensity * 0.08 * environment.sky.dusk_dawn_blend;
    let ambient_color = sky_lerp3(
        [0.018, 0.024, 0.056],
        cycle.base_ambient_color,
        (day_strength + environment.sky.dusk_dawn_blend * 0.28).clamp(0.0, 1.0),
    );
    let ambient_intensity = cycle.base_ambient_intensity
        * (0.08 + environment.lighting_intent.sky_light_intensity.clamp(0.0, 1.0))
        * (1.0 - environment.exposure_intent.storm_darkening.clamp(0.0, 0.75));
    SkyFrameSample {
        to_sun,
        sky_tint: sky_color_to_rgba(sky_clamp3(sky_rgb, 0.0, 1.0)),
        cloud_tint: sky_color_to_rgba(sky_clamp3(cloud_rgb, 0.0, 1.0)),
        sun_color: sky_clamp3(sun_color, 0.0, 1.0),
        sun_intensity: sun_intensity.max(0.0),
        ambient_color: sky_clamp3(ambient_color, 0.0, 1.0),
        ambient_intensity: ambient_intensity.max(0.0),
    }
}

fn apply_sky_visuals(world: &mut newengine_ecs::World, frame: SkyFrameSample, atmosphere: Option<SkyAtmosphereRuntime>) {
    let radius = atmosphere.as_ref().map(|a| a.radius).unwrap_or(220.0).max(16.0);

    let entities = world
        .query::<SkyVisualRuntime>()
        .map(|(entity, visual)| (entity, visual.kind))
        .collect::<Vec<_>>();

    for (entity, kind) in entities {
        match kind {
            SkyVisualKind::Dome => {
                if let Some(primitive) = world.get_mut_tracked::<Primitive>(entity) {
                    primitive.color = [
                        (frame.sky_tint[0] * 0.86 + frame.cloud_tint[0] * 0.14).clamp(0.0, 1.0),
                        (frame.sky_tint[1] * 0.86 + frame.cloud_tint[1] * 0.14).clamp(0.0, 1.0),
                        (frame.sky_tint[2] * 0.86 + frame.cloud_tint[2] * 0.14).clamp(0.0, 1.0),
                        1.0,
                    ];
                }
                if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
                    t.position = Vec3::ZERO;
                    t.scale = Vec3::splat(radius);
                }
            }
        }
    }
}

pub fn tick_game_ready_sky_cycle(world: &mut newengine_ecs::World, dt: f32) {
    let (frame, atmosphere, environment_frame) = {
        let atmosphere = world.resource::<SkyAtmosphereRuntime>().cloned();
        let Some(cycle) = world.resource_mut::<SkyCycleRuntime>() else {
            return;
        };

        let time_snapshot = time_snapshot_for_sky_cycle();
        if let Some(snapshot) = &time_snapshot {
            cycle.time_of_day_hours = (snapshot.game.normalized_day as f32 * 24.0).rem_euclid(24.0);
        } else if dt > 0.0 {
            log::debug!(
                "game-ready sky cycle: engine.time route required for animated time; authored scene.day_night time remains fixed while degraded"
            );
        }

        let snapshot = time_snapshot.unwrap_or_else(|| authored_time_snapshot_for_sky_cycle(cycle));
        let environment_frame = environment_frame_for_sky_cycle(cycle, snapshot);
        let frame = if let Some(environment) = environment_frame.as_ref() {
            sample_sky_frame_from_environment(cycle, environment)
        } else {
            let to_sun = solar_direction_from_cycle(
                cycle.time_of_day_hours,
                cycle.latitude_degrees,
                cycle.axial_tilt_degrees,
            );
            sample_sky_frame(cycle, atmosphere.as_ref(), to_sun)
        };
        (frame, atmosphere, environment_frame)
    };

    if let Some(environment_frame) = environment_frame {
        let visual_assets = environment_frame.visual_assets.clone();
        let changed = world
            .resource::<GameReadyEnvironmentVisualAssetsRuntime>()
            .map(|current| current.visual_assets != visual_assets)
            .unwrap_or(true);
        if changed {
            log::debug!(
                "game-ready environment bridge: visual asset group='{}' dictionary='{}' sky='{}' sun='{}' moon='{}' cloud_density='{}' weather='{}'",
                visual_assets.visual_group_id,
                visual_assets.texture_dictionary_ref,
                visual_assets.sky_texture_ref,
                visual_assets.sun_disk_texture_ref,
                visual_assets.moon_disk_texture_ref,
                visual_assets.cloud_density_texture_ref,
                visual_assets.weather_visual_ref
            );
        }
        world.insert_resource(GameReadyEnvironmentVisualAssetsRuntime { visual_assets });
        world.insert_resource(GameReadyEnvironmentFrameRuntime { frame: environment_frame });
    }

    if let Some(ambient) = world.resource_mut::<AmbientLight>() {
        ambient.color = frame.ambient_color;
        ambient.intensity = frame.ambient_intensity;
    }

    let direction = -frame.to_sun;
    let sun_entity = world.query::<DirectionalLight>().next().map(|(entity, _)| entity);
    if let Some(sun_entity) = sun_entity {
        if let Some(light) = world.get_mut_tracked::<DirectionalLight>(sun_entity) {
            light.direction_ws = [direction.x, direction.y, direction.z];
            light.color = frame.sun_color;
            light.intensity = frame.sun_intensity;
        }
    }

    world.insert_resource(SkyClearColorRuntime { color: frame.sky_tint });
    apply_sky_visuals(world, frame, atmosphere);
}


#[inline]
fn configure_game_ready_lighting(world: &mut newengine_ecs::World, spec: &GameReadyLightingSpec) {
    let ambient = AmbientLight {
        color: spec.ambient_color,
        intensity: spec.ambient_intensity,
    };
    match world.resource_mut::<AmbientLight>() {
        Some(a) => *a = ambient,
        None => world.insert_resource(ambient),
    }

    let sun_dir = Vec3::new(
        spec.sun_direction[0],
        spec.sun_direction[1],
        spec.sun_direction[2],
    )
    .normalize_or_zero();
    let sun = DirectionalLight {
        direction_ws: [sun_dir.x, sun_dir.y, sun_dir.z],
        color: spec.sun_color,
        intensity: spec.sun_intensity,
    };
    let sun_entity = world.query::<DirectionalLight>().next().map(|(entity, _)| entity);
    if let Some(sun_entity) = sun_entity {
        if let Some(light) = world.get_mut_tracked::<DirectionalLight>(sun_entity) {
            *light = sun;
        }
    } else {
        let sun_entity = spawn_named(world, "Game/Sun");
        let _ = world.insert(sun_entity, sun);
    }

    sync_game_ready_day_night_to_engine_time(&spec.day_night);

    world.insert_resource(SkyCycleRuntime {
        enabled: spec.day_night.enabled,
        time_of_day_hours: spec.day_night.time_of_day_hours,
        day_length_seconds: spec.day_night.day_length_seconds,
        latitude_degrees: spec.day_night.latitude_degrees,
        axial_tilt_degrees: spec.day_night.axial_tilt_degrees,
        base_sun_color: spec.sun_color,
        base_sun_intensity: spec.sun_intensity,
        base_ambient_color: spec.ambient_color,
        base_ambient_intensity: spec.ambient_intensity,
    });
    tick_game_ready_sky_cycle(world, 0.0);

    log::info!(
        "game-ready sky cycle: tod={:.2}h day_len={:.1}s ambient={:?}/{:.3} sun_dir={:?} sun={:?}/{:.3} shadows={} strength={:.3}",
        spec.day_night.time_of_day_hours,
        spec.day_night.day_length_seconds,
        ambient.color,
        ambient.intensity,
        sun.direction_ws,
        sun.color,
        sun.intensity,
        spec.shadows.enabled,
        spec.shadows.contact_strength,
    );

    world.insert_resource(ShadowSettings {
        enabled: spec.shadows.enabled,
        method: if spec.shadows.cascade_count > 1 {
            newengine_lighting::ShadowMethod::CascadedShadowMaps
        } else {
            newengine_lighting::ShadowMethod::DirectionalDepthMap
        },
        resolution: spec.shadows.resolution,
        cascade_count: spec.shadows.cascade_count,
        max_distance: spec.shadows.max_distance,
        softness: spec.shadows.softness,
        bias: spec.shadows.bias,
        normal_bias: spec.shadows.normal_bias,
        contact_strength: spec.shadows.contact_strength,
    });
}

