use crate::math::{circular_window, normalize, smoothstep, unit_noise};
use newengine_world_environment_api::{
    CelestialBodyDto, Color3Dto, TimeOfDayPhase, TimeOfDayStateDto, Vec3Dto,
};

pub(crate) fn time_of_day_state(tod: f32) -> TimeOfDayStateDto {
    const PHASES: &[(TimeOfDayPhase, f32, f32)] = &[
        (TimeOfDayPhase::Dawn, 0.25, 0.055),
        (TimeOfDayPhase::Dusk, 0.75, 0.055),
    ];
    let hours = tod.rem_euclid(1.0) * 24.0;
    let day_blend = sky_day_blend(tod);
    let night_blend = 1.0 - day_blend;
    let dawn = circular_window(tod, PHASES[0].1, PHASES[0].2);
    let dusk = circular_window(tod, PHASES[1].1, PHASES[1].2);
    let dawn_dusk_blend = (dawn + dusk).clamp(0.0, 1.0);
    let phase = PHASES
        .iter()
        .find(|(_, center, half_width)| circular_window(tod, *center, *half_width) > 0.18)
        .map(|(phase, _, _)| *phase)
        .unwrap_or_else(|| {
            if day_blend >= 0.50 {
                TimeOfDayPhase::Day
            } else {
                TimeOfDayPhase::Night
            }
        });
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

pub(crate) fn sun_body(tod: f32) -> CelestialBodyDto {
    let tau = std::f32::consts::TAU;
    let orbit = tau * (tod - 0.25);
    let altitude_raw = orbit.sin();
    let visibility = smoothstep(0.0, 0.08, altitude_raw.max(0.0));
    let direction = normalize(Vec3Dto::new(
        orbit.cos(),
        altitude_raw,
        (tau * tod).sin() * 0.35,
    ));
    CelestialBodyDto {
        direction_world: direction,
        altitude_radians: altitude_raw.asin(),
        azimuth_radians: tau * tod,
        angular_radius_radians: 0.00465,
        color_linear: mix_color_local(
            Color3Dto::new(1.0, 0.47, 0.22),
            Color3Dto::new(1.0, 0.95, 0.82),
            visibility,
        ),
        intensity_lux_hint: 105_000.0 * visibility,
        visible: visibility > 0.01,
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

fn sky_day_blend(tod: f32) -> f32 {
    let sun_height = (std::f32::consts::TAU * (tod - 0.25)).sin();
    smoothstep(-0.10, 0.22, sun_height)
}

fn mix_color_local(a: Color3Dto, b: Color3Dto, t: f32) -> Color3Dto {
    let t = t.clamp(0.0, 1.0);
    Color3Dto::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}
