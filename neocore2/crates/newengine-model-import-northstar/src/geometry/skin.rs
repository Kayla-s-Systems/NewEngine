fn decode_skin(
    pak: &PakFile,
    header: usize,
    source_vertex_indices: &[usize],
    mesh_name: &str,
) -> Result<(Vec<YddBinarySkinVertex>, SkinLossStats), String> {
    let map = pak
        .resolve_pointer(header + 16)?
        .ok_or_else(|| format!("skin map missing mesh='{mesh_name}'"))?;
    let weights = pak
        .resolve_pointer(header + 24)?
        .ok_or_else(|| format!("skin weights missing mesh='{mesh_name}'"))?;
    let profile = pak.read_u32(header + 8)?;
    if profile > 1 {
        return Err(format!(
            "unsupported source skin profile mesh='{mesh_name}' profile={profile}"
        ));
    }
    let mut out = Vec::with_capacity(source_vertex_indices.len());
    let mut stats = SkinLossStats::default();
    for &vertex in source_vertex_indices {
        let count = pak.read_u32(map + vertex * 8)? as usize;
        let relative = pak.read_u32(map + vertex * 8 + 4)? as usize;
        if count == 0 || count > 12 {
            return Err(format!(
                "unsupported source skin influence count mesh='{mesh_name}' vertex={vertex} count={count}"
            ));
        }
        let mut combined = BTreeMap::<u16, f32>::new();
        for influence in 0..count {
            let base = weights
                .checked_add(relative)
                .ok_or("skin weight address overflow")?;
            let (joint, weight) = match profile {
                0 => {
                    let packed = pak.read_u32(
                        base.checked_add(influence * 4)
                            .ok_or("packed skin weight address overflow")?,
                    )?;
                    (
                        (packed >> 22) as u16,
                        (packed & PACKED_WEIGHT_MASK) as f32 / PACKED_WEIGHT_DENOMINATOR,
                    )
                }
                1 => {
                    // NorthStar PC also uses an explicit 8-byte influence representation:
                    // f32 weight followed by u32 joint index. The profile bit at skin_header+8
                    // selects this layout. Treating these words as the packed 22/10-bit profile
                    // corrupts both weights and joints (notably Ellie backpack cloth/straps).
                    let influence_base = base
                        .checked_add(influence * 8)
                        .ok_or("explicit skin influence address overflow")?;
                    let weight = pak.read_f32(influence_base)?;
                    let joint = pak.read_u32(influence_base + 4)?;
                    let joint = u16::try_from(joint).map_err(|_| {
                        format!(
                            "explicit source skin joint exceeds u16 mesh='{mesh_name}' vertex={vertex} joint={joint}"
                        )
                    })?;
                    (joint, weight)
                }
                _ => unreachable!(),
            };
            if !weight.is_finite() || weight < 0.0 {
                return Err(format!(
                    "invalid source skin influence mesh='{mesh_name}' vertex={vertex} joint={joint} weight={weight}"
                ));
            }
            *combined.entry(joint).or_insert(0.0) += weight;
        }
        let mut influences = combined.into_iter().collect::<Vec<_>>();
        influences.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        let total = influences.iter().map(|(_, weight)| *weight).sum::<f32>();
        if !total.is_finite() || total <= 1.0e-8 {
            return Err(format!(
                "invalid source skin total mesh='{mesh_name}' vertex={vertex} total={total}"
            ));
        }
        let top4 = influences
            .iter()
            .take(4)
            .map(|(_, weight)| *weight)
            .sum::<f32>();
        let top8 = influences
            .iter()
            .take(8)
            .map(|(_, weight)| *weight)
            .sum::<f32>();
        let loss4 = (1.0 - top4 / total).clamp(0.0, 1.0);
        let loss8 = (1.0 - top8 / total).clamp(0.0, 1.0);
        stats.weighted_vertices += 1;
        stats.source_influences += influences.len() as u64;
        stats.max_source_influences = stats.max_source_influences.max(influences.len() as u32);
        stats.top4_loss_sum += loss4 as f64;
        stats.top4_loss_max = stats.top4_loss_max.max(loss4);
        stats.top8_loss_sum += loss8 as f64;
        stats.top8_loss_max = stats.top8_loss_max.max(loss8);

        let retained = top8.max(1.0e-8);
        let mut joints = [0u16; 8];
        let mut normalized = [0.0f32; 8];
        for (slot, (joint, weight)) in influences.into_iter().take(8).enumerate() {
            joints[slot] = joint;
            normalized[slot] = weight / retained;
        }
        out.push(YddBinarySkinVertex {
            joints: [joints[0], joints[1], joints[2], joints[3]],
            weights: [normalized[0], normalized[1], normalized[2], normalized[3]],
            joints_extra: [joints[4], joints[5], joints[6], joints[7]],
            weights_extra: [normalized[4], normalized[5], normalized[6], normalized[7]],
        });
    }
    Ok((out, stats))
}
