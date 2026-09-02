/// Build renderer-facing foliage commands without mutating scene/ECS state.
///
/// GPU culling is selected only when both advertised capabilities are present.
/// Otherwise the same request is resolved by the deterministic CPU fallback.
pub fn build_foliage_extraction_plan_v1(
    request: FoliageExtractionRequestV1,
) -> Result<FoliageExtractionPlanV1, String> {
    let settings = request.settings.sanitized()?;
    request.runtime_asset.validate()?;

    let use_gpu = settings.prefer_gpu_culling
        && request.capabilities.gpu_culling
        && request.capabilities.indirect_draw;
    let path = if use_gpu {
        FoliageExtractionPathV1::GpuIndirect
    } else {
        FoliageExtractionPathV1::CpuFallback
    };

    let mut instances = request.instances;
    instances.sort_by_key(|instance| instance.stable_id);
    let input_instances = instances.len().min(u32::MAX as usize) as u32;
    let mut density_rejected = 0u32;
    let mut distance_culled = 0u32;
    let mut gpu_candidates = Vec::new();
    let mut batches = BTreeMap::<(u16, String, u64), Vec<FoliageInstanceCommandV1>>::new();

    for instance in instances {
        if density_fraction(instance.stable_id, settings.seed) > settings.density {
            density_rejected = density_rejected.saturating_add(1);
            continue;
        }

        let material_variant = instance
            .material_variant
            .as_deref()
            .unwrap_or(&settings.material_variant);
        let material = request.runtime_asset.material_for_variant(material_variant);
        if !material.is_valid() {
            return Err(format!(
                "foliage material variant '{}' has no registry-backed material handle",
                material_variant
            ));
        }

        if use_gpu {
            gpu_candidates.push(FoliageGpuCandidateV1 {
                stable_id: instance.stable_id,
                transform_cols: instance.transform_cols,
                bounds_center: instance.bounds_center,
                bounds_radius: instance.bounds_radius.abs().max(0.001),
                material,
            });
            continue;
        }

        let distance = instance_distance(&instance, request.view.camera_position);
        let radius_world = instance_world_radius(&instance);
        if distance - radius_world > settings.max_distance(request.view.shadow_pass) {
            distance_culled = distance_culled.saturating_add(1);
            continue;
        }

        let lod_index = settings.selected_lod(
            distance,
            request.runtime_asset.lods.len().min(u16::MAX as usize) as u16,
        );
        let lod = request
            .runtime_asset
            .lods
            .iter()
            .find(|lod| lod.lod_index == lod_index)
            .or_else(|| request.runtime_asset.lods.last())
            .expect("validated runtime asset has at least one LOD");
        let fade = lod_fade(distance, lod.max_distance, settings.lod.crossfade_width);
        let key = (lod.lod_index, lod.drawable_ref.clone(), material.raw());
        batches
            .entry(key)
            .or_default()
            .push(FoliageInstanceCommandV1 {
                stable_id: instance.stable_id,
                transform_cols: instance.transform_cols,
                lod_index: lod.lod_index,
                lod_fade: fade,
            });
    }

    let batches = batches
        .into_iter()
        .map(
            |((_lod_index, drawable_ref, material), instances)| FoliageDrawBatchV1 {
                drawable_ref,
                material: MaterialId(material),
                instances,
            },
        )
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    if use_gpu {
        warnings.push(format!(
            "GPU foliage candidates require capability '{}'; CPU fallback remains available",
            FOLIAGE_GPU_CULLING_CAPABILITY_ID
        ));
    }

    let gpu_work = use_gpu.then(|| FoliageGpuWorkV1 {
        settings: settings.clone(),
        lods: request.runtime_asset.lods.clone(),
        view: request.view,
        candidates: gpu_candidates,
    });

    Ok(FoliageExtractionPlanV1 {
        schema: FOLIAGE_EXTRACTION_PLAN_SCHEMA.to_owned(),
        path,
        wind: settings.wind,
        batches,
        gpu_work,
        input_instances,
        density_rejected,
        distance_culled,
        warnings,
    })
}
