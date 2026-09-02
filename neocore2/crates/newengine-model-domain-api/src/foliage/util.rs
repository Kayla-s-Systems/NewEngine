fn normalize_logical_source(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned()
}

fn has_extension(value: &str, extension: &str) -> bool {
    value
        .rsplit_once('.')
        .map(|(_, actual)| actual.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

fn speedtree_importer_id(value: &str) -> Option<&'static str> {
    if has_extension(value, SPEEDTREE_SRT_EXTENSION) {
        Some(FOLIAGE_SRT_IMPORTER_ID)
    } else if has_extension(value, SPEEDTREE_SPM_EXTENSION) {
        Some(FOLIAGE_SPM_IMPORTER_ID)
    } else {
        None
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn normalized_direction(direction: [f32; 3]) -> [f32; 3] {
    let x = finite_or(direction[0], 0.78);
    let y = finite_or(direction[1], 0.0);
    let z = finite_or(direction[2], 0.62);
    let length_sq = x * x + y * y + z * z;
    if length_sq <= 1.0e-8 {
        return FoliageWindSettings::default().direction;
    }
    let inv_len = length_sq.sqrt().recip();
    [x * inv_len, y * inv_len, z * inv_len]
}

fn sanitize_lod_distances(values: Vec<f32>) -> Vec<f32> {
    let mut values = values
        .into_iter()
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.25, 16_384.0))
        .collect::<Vec<_>>();
    values.sort_by(f32::total_cmp);
    values.dedup_by(|left, right| (*left - *right).abs() <= 0.001);
    if values.is_empty() {
        FoliageLodSettings::default().mesh_distances
    } else {
        values
    }
}

fn identity_cols() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn density_fraction(stable_id: u64, seed: u64) -> f32 {
    let mut value = stable_id ^ seed ^ 0x9E37_79B9_7F4A_7C15;
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    ((value >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

fn instance_distance(instance: &FoliageInstanceInputV1, camera: [f32; 3]) -> f32 {
    let center = instance_world_center(instance);
    let dx = center[0] - camera[0];
    let dy = center[1] - camera[1];
    let dz = center[2] - camera[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn instance_world_center(instance: &FoliageInstanceInputV1) -> [f32; 3] {
    let cols = instance.transform_cols;
    let local = instance.bounds_center;
    [
        cols[3][0] + cols[0][0] * local[0] + cols[1][0] * local[1] + cols[2][0] * local[2],
        cols[3][1] + cols[0][1] * local[0] + cols[1][1] * local[1] + cols[2][1] * local[2],
        cols[3][2] + cols[0][2] * local[0] + cols[1][2] * local[1] + cols[2][2] * local[2],
    ]
}

fn instance_world_radius(instance: &FoliageInstanceInputV1) -> f32 {
    let axis_length = |column: [f32; 4]| {
        (column[0] * column[0] + column[1] * column[1] + column[2] * column[2]).sqrt()
    };
    let max_scale = axis_length(instance.transform_cols[0])
        .max(axis_length(instance.transform_cols[1]))
        .max(axis_length(instance.transform_cols[2]))
        .max(0.001);
    instance.bounds_radius.abs().max(0.001) * max_scale
}

fn lod_fade(distance: f32, max_distance: f32, width: f32) -> f32 {
    if !max_distance.is_finite() || width <= f32::EPSILON {
        return 1.0;
    }
    ((max_distance - distance) / width).clamp(0.0, 1.0)
}
