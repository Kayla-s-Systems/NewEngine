use crate::math::{normalize, smoothstep, unit_noise};
use newengine_world_environment_api::{
    CelestialBodyDto, Color3Dto, TimeOfDayPhase, TimeOfDayStateDto, Vec3Dto,
};

pub(crate) fn time_of_day_state(tod: f32, sun_height: f32) -> TimeOfDayStateDto {
    let hours = tod.rem_euclid(1.0) * 24.0;
    let astronomical = smoothstep(-0.3090, -0.2079, sun_height);
    let civil = smoothstep(-0.1045, 0.0349, sun_height);
    let day_blend = smoothstep(-0.0349, 0.1392, sun_height);
    let night_blend = 1.0 - astronomical;
    let dawn_dusk_blend =
        ((astronomical - day_blend) * 0.25 + (civil - day_blend) * 0.75).clamp(0.0, 1.0);
    let phase = if day_blend >= 0.55 {
        TimeOfDayPhase::Day
    } else if night_blend >= 0.72 {
        TimeOfDayPhase::Night
    } else if tod.rem_euclid(1.0) < 0.5 {
        TimeOfDayPhase::Dawn
    } else {
        TimeOfDayPhase::Dusk
    };
    TimeOfDayStateDto {
        normalized_day: tod,
        hours,
        phase,
        dawn_dusk_blend,
        day_blend,
        night_blend,
    }
}

pub(crate) fn moon_phase(seed: u64, day_index: u64) -> f32 {
    let cycle = ((day_index as f32 + unit_noise(seed, 0, 0xA55A_0001) * 29.53) / 29.53).fract();
    (0.5 - (cycle - 0.5).abs()) * 2.0
}

pub(crate) fn sun_body(
    tod: f32,
    latitude_degrees: f32,
    axial_tilt_degrees: f32,
    day_index: u64,
) -> CelestialBodyDto {
    let tau = std::f32::consts::TAU;
    let latitude = latitude_degrees.to_radians().clamp(-1.5533, 1.5533);
    let tilt = axial_tilt_degrees
        .to_radians()
        .clamp(0.0, std::f32::consts::FRAC_PI_6);
    let season = tau * ((day_index as f32 - 80.0) / 365.2422);
    let declination = tilt * season.sin();
    let hour_angle = tau * (tod.rem_euclid(1.0) - 0.5);
    let sin_altitude = (latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos())
    .clamp(-1.0, 1.0);
    let altitude = sin_altitude.asin();
    let east = declination.cos() * hour_angle.sin();
    let north =
        latitude.cos() * declination.sin() - latitude.sin() * declination.cos() * hour_angle.cos();
    let direction = normalize(Vec3Dto::new(east, sin_altitude, -north));
    let azimuth = east.atan2(north);

    let visible = smoothstep(-0.1045, 0.0349, sin_altitude);
    let photometric = sin_altitude.max(0.0).powf(0.42);
    let warm = (1.0 - smoothstep(0.035, 0.35, sin_altitude)).clamp(0.0, 1.0);
    CelestialBodyDto {
        direction_world: direction,
        altitude_radians: altitude,
        azimuth_radians: azimuth,
        angular_radius_radians: 0.00465,
        color_linear: mix_color_local(
            Color3Dto::new(1.0, 0.38, 0.12),
            Color3Dto::new(1.0, 0.95, 0.84),
            1.0 - warm,
        ),
        intensity_lux_hint: 105_000.0 * photometric + 420.0 * visible * (1.0 - photometric),
        visible: visible > 0.01,
    }
}

pub(crate) fn moon_body(tod: f32, seed: u64, day_index: u64) -> CelestialBodyDto {
    let tau = std::f32::consts::TAU;
    let phase_offset = moon_phase(seed, day_index) * 0.08;
    let orbit = tau * (tod + 0.25 + phase_offset);
    let altitude_raw = orbit.sin();
    let visibility = smoothstep(0.0, 0.08, altitude_raw.max(0.0));
    let phase_visibility = 0.20 + moon_phase(seed, day_index) * 0.80;
    CelestialBodyDto {
        direction_world: normalize(Vec3Dto::new(
            orbit.cos(),
            altitude_raw,
            (tau * (tod + 0.5)).sin() * 0.25,
        )),
        altitude_radians: altitude_raw.asin(),
        azimuth_radians: tau * (tod + 0.5),
        angular_radius_radians: 0.00450,
        color_linear: Color3Dto::new(0.58, 0.66, 0.86),
        intensity_lux_hint: 0.25 * visibility * phase_visibility,
        visible: visibility > 0.01,
    }
}

fn mix_color_local(a: Color3Dto, b: Color3Dto, t: f32) -> Color3Dto {
    let t = t.clamp(0.0, 1.0);
    Color3Dto::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}
