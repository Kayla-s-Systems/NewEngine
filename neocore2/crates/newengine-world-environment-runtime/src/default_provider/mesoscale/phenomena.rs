use std::collections::VecDeque;

use newengine_world_api::WorldCellCoord;
use newengine_world_environment_api::{
    AabbDto, EnvironmentAtmosphereCellDto, EnvironmentObjectDto, EnvironmentObjectId,
    EnvironmentObjectKind, PrecipitationKind, TransformDto, Vec3Dto,
};

use super::topology::GridTopology;
use crate::profile_catalog::AtmosphereProfileDescriptor;

pub(super) fn extract(
    cells: &[EnvironmentAtmosphereCellDto],
    topology: &GridTopology,
    cell_size_m: f32,
    atmosphere_profile: &AtmosphereProfileDescriptor,
) -> Vec<EnvironmentObjectDto> {
    if cells.is_empty() || cell_size_m <= 1.0 {
        return Vec::new();
    }
    let mut objects = Vec::new();
    objects.extend(components(
        cells,
        topology,
        cell_size_m,
        EnvironmentObjectKind::CloudField,
        |cell| cell.clouds.coverage > 0.18 && cell.atmosphere.cloud_water_path_kg_m2 > 0.025,
    ));
    objects.extend(components(
        cells,
        topology,
        cell_size_m,
        EnvironmentObjectKind::StormCell,
        |cell| {
            cell.weather.thunder.probability > 0.14
                && cell.weather.precipitation.rate_mm_per_hour > 0.25
                && cell.atmosphere.cape_j_per_kg > 200.0
        },
    ));
    objects.extend(components(
        cells,
        topology,
        cell_size_m,
        EnvironmentObjectKind::FogBank,
        |cell| {
            cell.atmosphere.fog_density > 0.24
                && cell.atmosphere.visibility_distance_meters < 4000.0
        },
    ));
    objects.extend(components(
        cells,
        topology,
        cell_size_m,
        EnvironmentObjectKind::SnowBand,
        |cell| {
            matches!(cell.weather.precipitation.kind, PrecipitationKind::Snow)
                && cell.weather.precipitation.rate_mm_per_hour > 0.12
        },
    ));
    objects.extend(components(
        cells,
        topology,
        cell_size_m,
        EnvironmentObjectKind::DustWall,
        |cell| {
            cell.surface.moisture_availability < 0.15
                && cell.atmosphere.aerosol_density > 0.45
                && cell.wind.global_speed_mps > 7.0
        },
    ));
    objects.extend(front_faces(
        cells,
        topology,
        cell_size_m,
        atmosphere_profile,
    ));
    objects.sort_by_key(|object| object.id);
    objects
}

fn components<F>(
    cells: &[EnvironmentAtmosphereCellDto],
    topology: &GridTopology,
    cell_size_m: f32,
    kind: EnvironmentObjectKind,
    predicate: F,
) -> Vec<EnvironmentObjectDto>
where
    F: Fn(&EnvironmentAtmosphereCellDto) -> bool,
{
    let mut active = cells.iter().map(&predicate).collect::<Vec<_>>();
    let mut visited = vec![false; cells.len()];
    let mut out = Vec::new();
    for start in 0..cells.len() {
        if !active[start] || visited[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        let mut indices = Vec::new();
        visited[start] = true;
        while let Some(index) = queue.pop_front() {
            indices.push(index);
            let coord = cells[index].cell;
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let neighbor = WorldCellCoord::new(coord.x + dx, coord.z + dz);
                let Some(&next) = topology.index_by_cell.get(&neighbor) else {
                    continue;
                };
                if active[next] && !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        if !indices.is_empty() {
            out.push(component_object(cells, &indices, cell_size_m, kind));
        }
    }
    // Preserve the explicit boolean buffer as a distinct topology input. This also
    // makes it impossible for a presentation label to join a component implicitly.
    active.clear();
    out
}

fn component_object(
    cells: &[EnvironmentAtmosphereCellDto],
    indices: &[usize],
    cell_size_m: f32,
    kind: EnvironmentObjectKind,
) -> EnvironmentObjectDto {
    let mut owning_cells = indices.iter().map(|&i| cells[i].cell).collect::<Vec<_>>();
    owning_cells.sort();
    let mut min = Vec3Dto::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = Vec3Dto::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut max_cape = 0.0_f32;
    let mut max_precip = 0.0_f32;
    let mut max_thunder = 0.0_f32;
    let mut max_cwp = 0.0_f32;
    let mut min_visibility = f32::INFINITY;
    for &index in indices {
        let cell = &cells[index];
        let x0 = cell.cell.x as f32 * cell_size_m;
        let x1 = (cell.cell.x as f32 + 1.0) * cell_size_m;
        let z0 = cell.cell.z as f32 * cell_size_m;
        let z1 = (cell.cell.z as f32 + 1.0) * cell_size_m;
        let terrain = cell.surface.terrain_elevation_meters;
        let (y0, y1) = vertical_bounds(cell, kind, terrain);
        min.x = min.x.min(x0);
        min.y = min.y.min(y0);
        min.z = min.z.min(z0);
        max.x = max.x.max(x1);
        max.y = max.y.max(y1);
        max.z = max.z.max(z1);
        max_cape = max_cape.max(cell.atmosphere.cape_j_per_kg);
        max_precip = max_precip.max(cell.weather.precipitation.rate_mm_per_hour);
        max_thunder = max_thunder.max(cell.weather.thunder.probability);
        max_cwp = max_cwp.max(cell.atmosphere.cloud_water_path_kg_m2);
        min_visibility = min_visibility.min(cell.atmosphere.visibility_distance_meters);
    }
    let center = Vec3Dto::new(
        0.5 * (min.x + max.x),
        0.5 * (min.y + max.y),
        0.5 * (min.z + max.z),
    );
    EnvironmentObjectDto {
        id: stable_component_id(kind, &owning_cells),
        kind,
        bounds: AabbDto { min, max },
        owning_cells,
        transform: TransformDto {
            translation: center,
            ..TransformDto::default()
        },
        tags: vec![
            "source.mesoscale_physics".to_owned(),
            object_cause_tag(kind).to_owned(),
        ],
        state_json: serde_json::json!({
            "source": "mesoscale.connected_component",
            "reason": "connected physical cells satisfy object diagnostics",
            "priority": if matches!(kind, EnvironmentObjectKind::StormCell) { "critical" } else { "normal" },
            "max_cape_j_kg": max_cape,
            "max_precipitation_mm_h": max_precip,
            "max_thunder_probability": max_thunder,
            "max_cwp_kg_m2": max_cwp,
            "min_visibility_m": min_visibility,
        }),
    }
}

fn vertical_bounds(
    cell: &EnvironmentAtmosphereCellDto,
    kind: EnvironmentObjectKind,
    terrain: f32,
) -> (f32, f32) {
    match kind {
        EnvironmentObjectKind::CloudField | EnvironmentObjectKind::StormCell => (
            terrain + cell.atmosphere.lifting_condensation_level_meters,
            terrain + cell.atmosphere.convective_cloud_top_meters.max(500.0),
        ),
        EnvironmentObjectKind::FogBank => (terrain, terrain + 500.0),
        EnvironmentObjectKind::SnowBand => (terrain, terrain + 1800.0),
        EnvironmentObjectKind::DustWall => (terrain, terrain + 2200.0),
        _ => (terrain, terrain + 1500.0),
    }
}

fn front_faces(
    cells: &[EnvironmentAtmosphereCellDto],
    topology: &GridTopology,
    cell_size_m: f32,
    atmosphere_profile: &AtmosphereProfileDescriptor,
) -> Vec<EnvironmentObjectDto> {
    let mut out = Vec::new();
    for &(a, b, nx, nz) in &topology.faces {
        let left = &cells[a];
        let right = &cells[b];
        let p_left = sea_level_pressure_hpa(left);
        let p_right = sea_level_pressure_hpa(right);
        let dp = (p_right - p_left).abs();
        // Compare air masses on a common reference surface. Raw surface T/q across
        // terrain are vertical-profile differences and must not synthesize a front.
        let t_left_ref = reference_temperature_c(left, atmosphere_profile);
        let t_right_ref = reference_temperature_c(right, atmosphere_profile);
        let q_left_ref = reference_specific_humidity_g_kg(left, atmosphere_profile);
        let q_right_ref = reference_specific_humidity_g_kg(right, atmosphere_profile);
        let dt = (t_right_ref - t_left_ref).abs();
        let dq = (q_right_ref - q_left_ref).abs();
        let gradient_hpa_100km = dp * 100_000.0 / cell_size_m.max(1.0);
        let front_strength = (gradient_hpa_100km / 4.0).max(dt / 5.0).max(dq / 3.0);
        if front_strength < 0.65 {
            continue;
        }
        let (lx, lz) = left.cell.center_xz(cell_size_m, cell_size_m);
        let (rx, rz) = right.cell.center_xz(cell_size_m, cell_size_m);
        let cx = 0.5 * (lx + rx);
        let cz = 0.5 * (lz + rz);
        let half_thin = cell_size_m * 0.08;
        let half_long = cell_size_m * 0.50;
        let (half_x, half_z) = if nx != 0 {
            (half_thin, half_long)
        } else {
            (half_long, half_thin)
        };
        let terrain = left
            .surface
            .terrain_elevation_meters
            .min(right.surface.terrain_elevation_meters);
        let top = (left
            .atmosphere
            .convective_cloud_top_meters
            .max(right.atmosphere.convective_cloud_top_meters)
            .max(1800.0))
            + left
                .surface
                .terrain_elevation_meters
                .max(right.surface.terrain_elevation_meters);
        let owning_cells = vec![left.cell, right.cell];
        out.push(EnvironmentObjectDto {
            id: stable_front_id(left.cell, right.cell),
            kind: EnvironmentObjectKind::WeatherFront,
            bounds: AabbDto {
                min: Vec3Dto::new(cx - half_x, terrain, cz - half_z),
                max: Vec3Dto::new(cx + half_x, top, cz + half_z),
            },
            owning_cells,
            transform: TransformDto {
                translation: Vec3Dto::new(cx, 0.5 * (terrain + top), cz),
                ..TransformDto::default()
            },
            tags: vec![
                "source.mesoscale_physics".to_owned(),
                "cause.air_mass_gradient".to_owned(),
            ],
            state_json: serde_json::json!({
                "source": "mesoscale.face_gradient",
                "reason": "adjacent physical air masses have a resolved thermodynamic/pressure gradient",
                "priority": "normal",
                "delta_pressure_hpa": dp,
                "pressure_gradient_hpa_per_100km": gradient_hpa_100km,
                "delta_temperature_c": dt,
                "delta_specific_humidity_g_kg": dq,
                "front_strength": front_strength,
                "face_normal_x": nx,
                "face_normal_z": nz,
            }),
        });
    }
    out
}

fn sea_level_pressure_hpa(cell: &EnvironmentAtmosphereCellDto) -> f32 {
    let q = (cell.atmosphere.specific_humidity_g_per_kg * 0.001).clamp(0.0, 0.04);
    let virtual_temperature_k = (cell.atmosphere.temperature_celsius + 273.15) * (1.0 + 0.61 * q);
    let scale_height = 287.05 * virtual_temperature_k.max(190.0) / 9.80665;
    cell.atmosphere.surface_pressure_hpa
        * (cell.surface.terrain_elevation_meters.clamp(-500.0, 7000.0) / scale_height.max(4500.0))
            .exp()
}

fn reference_temperature_c(
    cell: &EnvironmentAtmosphereCellDto,
    profile: &AtmosphereProfileDescriptor,
) -> f32 {
    cell.atmosphere.temperature_celsius
        + profile.lapse_rate_k_per_km
            * (cell.surface.terrain_elevation_meters - profile.terrain_elevation_m)
            * 0.001
}

fn reference_specific_humidity_g_kg(
    cell: &EnvironmentAtmosphereCellDto,
    profile: &AtmosphereProfileDescriptor,
) -> f32 {
    cell.atmosphere.specific_humidity_g_per_kg
        * ((cell.surface.terrain_elevation_meters - profile.terrain_elevation_m) / 2500.0).exp()
}

fn stable_component_id(
    kind: EnvironmentObjectKind,
    cells: &[WorldCellCoord],
) -> EnvironmentObjectId {
    let mut hash = 0xcbf29ce484222325_u64 ^ kind_code(kind);
    for cell in cells {
        hash = fnv(hash, cell.x as u32 as u64);
        hash = fnv(hash, cell.z as u32 as u64);
    }
    EnvironmentObjectId { stable_id: hash }
}

fn stable_front_id(a: WorldCellCoord, b: WorldCellCoord) -> EnvironmentObjectId {
    let mut pair = [a, b];
    pair.sort();
    stable_component_id(EnvironmentObjectKind::WeatherFront, &pair)
}

#[inline]
fn fnv(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(0x100000001b3)
}

fn kind_code(kind: EnvironmentObjectKind) -> u64 {
    match kind {
        EnvironmentObjectKind::CloudField => 1,
        EnvironmentObjectKind::CloudVolume => 2,
        EnvironmentObjectKind::StormCell => 3,
        EnvironmentObjectKind::FogBank => 4,
        EnvironmentObjectKind::WeatherFront => 5,
        EnvironmentObjectKind::DustWall => 6,
        EnvironmentObjectKind::SnowBand => 7,
        EnvironmentObjectKind::HeatHazeZone => 8,
    }
}

fn object_cause_tag(kind: EnvironmentObjectKind) -> &'static str {
    match kind {
        EnvironmentObjectKind::CloudField => "cause.condensate",
        EnvironmentObjectKind::StormCell => "cause.cape_mixed_phase_precipitation",
        EnvironmentObjectKind::FogBank => "cause.saturated_boundary_layer",
        EnvironmentObjectKind::SnowBand => "cause.snow_hydrometeor_flux",
        EnvironmentObjectKind::DustWall => "cause.dry_surface_aerosol_transport",
        _ => "cause.physical_state",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_depend_on_topology_not_seed_or_frame_number() {
        let cells = [WorldCellCoord::new(2, -1), WorldCellCoord::new(3, -1)];
        assert_eq!(
            stable_component_id(EnvironmentObjectKind::StormCell, &cells),
            stable_component_id(EnvironmentObjectKind::StormCell, &cells)
        );
    }
}
