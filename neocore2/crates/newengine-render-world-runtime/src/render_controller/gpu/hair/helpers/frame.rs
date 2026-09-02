#[allow(clippy::too_many_arguments)]
pub(super) fn encode_frame_ubo(
    view_projection: Mat4,
    camera_position: Vec3,
    camera_right: Vec3,
    camera_up: Vec3,
    dt: f32,
    directional_dir_intensity: [f32; 4],
    directional_color: [f32; 4],
    ambient_color: [f32; 4],
    counts: HairTopologyCounts,
    read_point_base: usize,
    write_point_base: usize,
    camera_forward: Vec3,
    shadow_frame: ShadowFrame,
    shadow_extent: Extent2D,
    shadow_enabled: bool,
) -> [u8; HAIR_FRAME_UBO_BYTES as usize] {
    let mut values = [0.0f32; HAIR_FRAME_UBO_BYTES as usize / 4];
    values[0..16].copy_from_slice(&view_projection.to_cols_array());
    values[16..20].copy_from_slice(&[camera_position.x, camera_position.y, camera_position.z, dt]);
    values[20..24].copy_from_slice(&[camera_right.x, camera_right.y, camera_right.z, 0.0]);
    values[24..28].copy_from_slice(&[camera_up.x, camera_up.y, camera_up.z, 0.0]);
    values[28..32].copy_from_slice(&directional_dir_intensity);
    values[32..36].copy_from_slice(&directional_color);
    values[36..40].copy_from_slice(&ambient_color);
    values[40..44].copy_from_slice(&[
        counts.point_count as f32,
        counts.strand_count as f32,
        counts.render_segment_count as f32,
        counts.rendered_strand_count as f32,
    ]);
    values[44..48].copy_from_slice(&[
        read_point_base as f32,
        write_point_base as f32,
        STRAND_BASE as f32,
        SEGMENT_BASE as f32,
    ]);
    values[48..52].copy_from_slice(&[
        INSTANCE_BASE as f32,
        HAIR_INSTANCE_SLOT_COUNT as f32,
        HAIR_SLOT_CAPACITY as f32,
        0.0,
    ]);

    let cascade_count = shadow_frame
        .cascade_count
        .clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
    let shadows_active = shadow_enabled && shadow_frame.params[0] >= 0.5;
    values[52..56].copy_from_slice(&[
        f32::from(shadows_active),
        cascade_count as f32,
        shadow_frame.params[2].clamp(0.0, 1.0),
        shadow_frame.params[1].max(0.0),
    ]);
    let atlas_w = shadow_extent.width.max(1) as f32;
    let atlas_h = shadow_extent.height.max(1) as f32;
    values[56..60].copy_from_slice(&[atlas_w, atlas_h, 1.0 / atlas_w, 1.0 / atlas_h]);
    values[60..64].copy_from_slice(&shadow_frame.cascade_splits);

    let mut biases = [0.0f32; MAX_DIRECTIONAL_SHADOW_CASCADES];
    for (index, bias) in biases.iter_mut().enumerate().take(cascade_count) {
        *bias = hair_shadow_receiver_bias(shadow_frame, index, shadow_extent);
    }
    values[64..68].copy_from_slice(&biases);

    let forward = camera_forward.normalize_or_zero();
    values[68..72].copy_from_slice(&[forward.x, forward.y, forward.z, 0.0]);
    let mut matrix_offset = 72usize;
    for cascade_index in 0..MAX_DIRECTIONAL_SHADOW_CASCADES {
        values[matrix_offset..matrix_offset + 16].copy_from_slice(
            &shadow_frame
                .cascade(cascade_index)
                .light_mvp
                .to_cols_array(),
        );
        matrix_offset += 16;
    }
    debug_assert_eq!(matrix_offset, 136);
    f32_array_bytes(values)
}
