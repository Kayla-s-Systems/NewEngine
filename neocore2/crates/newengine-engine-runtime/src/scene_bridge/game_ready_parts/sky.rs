// Sky lifecycle owns the canonical runtime model for atmospheric state,
// celestial visuals and scene lighting. Other GameReady modules spawn or
// read these components; they do not recalculate time-of-day colors locally.

#[derive(Clone, Debug)]
pub(crate) struct SkyDomeRuntime {
    pub follow_camera: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkyVisualKind {
    Dome,
    SunDisk,
    MoonDisk,
}

#[derive(Clone, Debug)]
pub(crate) struct SkyVisualRuntime {
    pub kind: SkyVisualKind,
}

#[derive(Clone, Debug)]
pub(crate) struct SkyAtmosphereRuntime {
    pub radius: f32,
    pub sun_radius: f32,
    pub moon_radius: f32,
    pub profile: GameReadySkyAtmosphereSpec,
}

pub(super) const SKY_VISUAL_SPAWN_ORDER: [SkyVisualKind; 3] = [
    SkyVisualKind::Dome,
    SkyVisualKind::SunDisk,
    SkyVisualKind::MoonDisk,
];

impl SkyVisualKind {
    #[inline]
    pub(super) fn entity_name(self) -> &'static str {
        match self {
            SkyVisualKind::Dome => "Sky/Imported-SkyDome",
            SkyVisualKind::SunDisk => "Sky/Sun-Disk",
            SkyVisualKind::MoonDisk => "Sky/Moon-Disk",
        }
    }

    #[inline]
    pub(super) fn initial_color(self, dome_color: [f32; 4]) -> [f32; 4] {
        match self {
            SkyVisualKind::Dome => dome_color,
            SkyVisualKind::SunDisk => [1.0, 0.82, 0.36, 1.0],
            SkyVisualKind::MoonDisk => [0.70, 0.76, 1.0, 1.0],
        }
    }

    #[inline]
    pub(super) fn initial_radius(self, spec: &GameReadySkySpec) -> f32 {
        match self {
            SkyVisualKind::Dome => spec.radius,
            SkyVisualKind::SunDisk => spec.sun_radius,
            SkyVisualKind::MoonDisk => spec.moon_radius,
        }
    }

    #[inline]
    pub(super) fn follows_camera(self, spec: &GameReadySkySpec) -> bool {
        match self {
            SkyVisualKind::Dome => spec.follow_camera,
            SkyVisualKind::SunDisk | SkyVisualKind::MoonDisk => true,
        }
    }

    #[inline]
    pub(super) fn primitive_id(self, dome_primitive_id: PrimitiveId) -> PrimitiveId {
        match self {
            SkyVisualKind::Dome => dome_primitive_id,
            SkyVisualKind::SunDisk | SkyVisualKind::MoonDisk => newengine_primitives::builtins::ID_DISC,
        }
    }
}

#[inline]
pub(super) fn sky_atmosphere_from_spec(spec: &GameReadySkySpec) -> SkyAtmosphereRuntime {
    SkyAtmosphereRuntime {
        radius: spec.radius,
        sun_radius: spec.sun_radius,
        moon_radius: spec.moon_radius,
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
    to_moon: Vec3,
    day: f32,
    twilight: f32,
    night: f32,
    sky_tint: [f32; 4],
    cloud_tint: [f32; 4],
    sun_color: [f32; 3],
    sun_intensity: f32,
    moon_color: [f32; 3],
    moon_intensity: f32,
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
                log::warn!("game-ready sky cycle: engine.time snapshot invalid; using frame dt projection for this tick err='{e}'");
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            log::warn!("game-ready sky cycle: engine.time snapshot unavailable; using frame dt projection for this tick err='{e}'");
            None
        }
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
    let to_moon = -to_sun;
    let elevation = to_sun.y;
    let moon_elevation = to_moon.y;

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

    let moon_above_horizon = sky_smoothstep(-0.08, 0.22, moon_elevation);
    let moon_intensity = (0.34 * night * moon_above_horizon + 0.06 * twilight * moon_above_horizon).clamp(0.0, 0.45);
    let moon_color = [0.50, 0.56, 0.76];

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
        to_moon,
        day,
        twilight,
        night,
        sky_tint: sky_color_to_rgba(sky_clamp3(sky_rgb, 0.0, 1.0)),
        cloud_tint: sky_color_to_rgba(sky_clamp3(cloud_rgb, 0.0, 1.0)),
        sun_color: sky_clamp3(sun_color, 0.0, 1.0),
        sun_intensity: sun_intensity.max(0.0),
        moon_color,
        moon_intensity,
        ambient_color: sky_clamp3(ambient_color, 0.0, 1.0),
        ambient_intensity: ambient_intensity.max(0.0),
    }
}

fn orient_disc_towards(direction: Vec3) -> Quat {
    let normal = sky_safe_dir(-direction, Vec3::new(0.0, -1.0, 0.0));
    Quat::from_rotation_arc(Vec3::Y, normal)
}

fn apply_sky_visuals(world: &mut newengine_ecs::World, frame: SkyFrameSample, atmosphere: Option<SkyAtmosphereRuntime>) {
    let radius = atmosphere.as_ref().map(|a| a.radius).unwrap_or(220.0).max(16.0);
    let sun_radius = atmosphere.as_ref().map(|a| a.sun_radius).unwrap_or(18.0).clamp(1.0, 64.0);
    let moon_radius = atmosphere.as_ref().map(|a| a.moon_radius).unwrap_or(13.5).clamp(1.0, 64.0);
    let sky_distance = radius * 0.82;
    let moon_distance = radius * 0.78;

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
            SkyVisualKind::SunDisk => {
                let visibility = (frame.day + frame.twilight * 0.72).clamp(0.0, 1.0);
                let bloom = (0.28 + frame.sun_intensity * 0.16).clamp(0.0, 1.0);
                let color = [
                    (frame.sun_color[0] * visibility * bloom).clamp(0.0, 1.0),
                    (frame.sun_color[1] * visibility * bloom).clamp(0.0, 1.0),
                    (frame.sun_color[2] * visibility * bloom).clamp(0.0, 1.0),
                    visibility.clamp(0.0, 1.0),
                ];
                if let Some(primitive) = world.get_mut_tracked::<Primitive>(entity) {
                    primitive.color = color;
                }
                if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
                    t.position = frame.to_sun * sky_distance;
                    t.rotation = orient_disc_towards(frame.to_sun);
                    t.scale = Vec3::splat(sun_radius * (0.82 + frame.twilight * 0.26));
                }
            }
            SkyVisualKind::MoonDisk => {
                let visibility = (frame.night + frame.twilight * 0.28).clamp(0.0, 1.0);
                let color = [
                    (frame.moon_color[0] * (0.18 + frame.moon_intensity * 1.85) * visibility).clamp(0.0, 1.0),
                    (frame.moon_color[1] * (0.18 + frame.moon_intensity * 1.85) * visibility).clamp(0.0, 1.0),
                    (frame.moon_color[2] * (0.18 + frame.moon_intensity * 1.85) * visibility).clamp(0.0, 1.0),
                    visibility,
                ];
                if let Some(primitive) = world.get_mut_tracked::<Primitive>(entity) {
                    primitive.color = color;
                }
                if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
                    t.position = frame.to_moon * moon_distance;
                    t.rotation = orient_disc_towards(frame.to_moon);
                    t.scale = Vec3::splat(moon_radius);
                }
            }
        }
    }
}

pub fn tick_game_ready_sky_cycle(world: &mut newengine_ecs::World, dt: f32) {
    let (frame, atmosphere) = {
        let atmosphere = world.resource::<SkyAtmosphereRuntime>().cloned();
        let Some(cycle) = world.resource_mut::<SkyCycleRuntime>() else {
            return;
        };

        if let Some(snapshot) = time_snapshot_for_sky_cycle() {
            cycle.time_of_day_hours = (snapshot.game.normalized_day as f32 * 24.0).rem_euclid(24.0);
            log::trace!(
                "game-ready sky cycle: time source='engine.time' frame={} normalized_day={:.6} tod_hours={:.3}",
                snapshot.frame_index,
                snapshot.game.normalized_day,
                cycle.time_of_day_hours
            );
        } else {
            let advance = if cycle.enabled && cycle.day_length_seconds > 0.0 {
                dt.max(0.0) * 24.0 / cycle.day_length_seconds
            } else {
                0.0
            };
            cycle.time_of_day_hours = (cycle.time_of_day_hours + advance).rem_euclid(24.0);
        }

        let to_sun = solar_direction_from_cycle(
            cycle.time_of_day_hours,
            cycle.latitude_degrees,
            cycle.axial_tilt_degrees,
        );
        let frame = sample_sky_frame(cycle, atmosphere.as_ref(), to_sun);
        (frame, atmosphere)
    };

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

