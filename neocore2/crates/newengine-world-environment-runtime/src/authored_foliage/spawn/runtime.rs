pub fn defer_foliage_prefabs(
    world: &mut newengine_ecs::World,
    root: EntityId,
    terrain: EntityId,
    terrain_surface: Option<TerrainSurfaceSampler>,
    materials: AuthoredEnvironmentMaterials,
    material_specs: &AuthoredEnvironmentMaterialSetSpec,
    palette: &AuthoredEnvironmentPaletteSpec,
    foliage: &AuthoredFoliageSpec,
    prefabs: &[AuthoredWorldPlacementSpec],
    player_start: Vec3,
) {
    world.insert_resource(DeferredFoliageSpawn {
        root,
        terrain,
        terrain_surface,
        materials,
        material_specs: material_specs.clone(),
        palette: palette.clone(),
        foliage: foliage.clone(),
        prefabs: prefabs.to_vec(),
        player_start,
    });
    newengine_ulog_api::ulog::info!(
        "authored-environment foliage placement deferred: prefab='{}' policy='authored static world -> collision admission -> ground ray -> instance'",
        foliage.prefab,
    );
}

pub fn tick_deferred_foliage_prefabs(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) {
    let ready = world
        .resource::<newengine_engine_runtime::gameplay::WorldAssemblyProgress>()
        .map(|progress| progress.is_ready())
        .unwrap_or(false);
    if !ready {
        return;
    }
    let Some(pending) = world.remove_resource::<DeferredFoliageSpawn>() else {
        return;
    };
    spawn_foliage_prefabs(
        world,
        prims,
        mats,
        pending.root,
        pending.terrain,
        pending.terrain_surface.as_ref(),
        pending.materials,
        &pending.material_specs,
        &pending.palette,
        &pending.foliage,
        &pending.prefabs,
        pending.player_start,
    );
}

pub fn spawn_foliage_prefabs(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    terrain: EntityId,
    terrain_surface: Option<&TerrainSurfaceSampler>,
    materials: AuthoredEnvironmentMaterials,
    material_specs: &AuthoredEnvironmentMaterialSetSpec,
    palette: &AuthoredEnvironmentPaletteSpec,
    foliage: &AuthoredFoliageSpec,
    prefabs: &[AuthoredWorldPlacementSpec],
    player_start: Vec3,
) {
    let Some(prefab) = choose_foliage_prefab(prefabs, &foliage.prefab) else {
        if foliage.enabled {
            newengine_ulog_api::ulog::warn!(
                "authored-environment: foliage enabled but prefab id='{}' is not declared or disabled",
                foliage.prefab
            );
        }
        return;
    };

    let alternate_prefab = if foliage.alternate_weight > 0.0
        && !foliage.alternate_prefab.trim().is_empty()
    {
        match choose_foliage_prefab(prefabs, &foliage.alternate_prefab) {
            Some(value) => Some(value),
            None => {
                newengine_ulog_api::ulog::warn!(
                    "authored-environment: alternate foliage prefab id='{}' is not declared or disabled; using primary='{}' only",
                    foliage.alternate_prefab,
                    prefab.id,
                );
                None
            }
        }
    } else {
        None
    };

    if !foliage.settings.canonical_path.is_empty() {
        let gateway = newengine_model_client::ModelGatewayClient::new(
            newengine_plugin_host::default_host_api(),
        );
        let request = newengine_model_domain_api::FoliageImportRequestV1 {
            settings: foliage.settings.clone(),
            ..newengine_model_domain_api::FoliageImportRequestV1::default()
        };
        match gateway.import_foliage(&request) {
            Ok(response) => {
                newengine_ulog_api::ulog::info!(
                    "authored-environment foliage source accepted: source='{}' runtime='{}' importer='{}' asset_id='{}' queue='{}' material_variant='{}' wind_enabled={} wind_strength={:.3} cull={:.1} shadow_cull={:.1}",
                    response.canonical_source_ref,
                    response.runtime_asset_ref,
                    response.importer_id,
                    response.asset_id,
                    response.queue_status,
                    foliage.settings.material_variant,
                    foliage.settings.wind.enabled,
                    foliage.settings.wind.strength,
                    foliage.settings.cull.max_distance,
                    foliage.settings.cull.shadow_max_distance,
                );
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "authored-environment foliage source import unavailable: source='{}' err='{}'; continuing with compiled prefab='{}' policy='licensed importer provider is optional at runtime; CPU/YDD fallback remains active'",
                    foliage.settings.canonical_path,
                    error,
                    prefab.source,
                );
            }
        }
    }

    if alternate_prefab.is_some() && !foliage.alternate_canonical_path.is_empty() {
        let gateway = newengine_model_client::ModelGatewayClient::new(
            newengine_plugin_host::default_host_api(),
        );
        let mut alternate_settings = foliage.settings.clone();
        alternate_settings.canonical_path = foliage.alternate_canonical_path.clone();
        let request = newengine_model_domain_api::FoliageImportRequestV1 {
            settings: alternate_settings,
            ..newengine_model_domain_api::FoliageImportRequestV1::default()
        };
        match gateway.import_foliage(&request) {
            Ok(response) => newengine_ulog_api::ulog::info!(
                "authored-environment alternate foliage source accepted: source='{}' runtime='{}' importer='{}' prefab='{}' weight={:.3}",
                response.canonical_source_ref,
                response.runtime_asset_ref,
                response.importer_id,
                foliage.alternate_prefab,
                foliage.alternate_weight,
            ),
            Err(error) => newengine_ulog_api::ulog::warn!(
                "authored-environment alternate foliage source import unavailable: source='{}' err='{}'; compiled prefab fallback remains active",
                foliage.alternate_canonical_path,
                error,
            ),
        }
    }

    let runtime_parts = match ensure_runtime_prefab_parts(
        prims,
        mats,
        prefab,
        materials,
        material_specs,
        palette,
    ) {
        Ok(parts) => parts,
        Err(e) => {
            newengine_ulog_api::ulog::error!(
                "authored-environment: prefab id='{}' source='{}' proxy='{}' failed to load .ydd runtime mesh through AssetManager; foliage skipped err='{}'",
                prefab.id,
                prefab.source,
                prefab.proxy,
                e
            );
            return;
        }
    };

    let alternate_runtime_parts = if let Some(alternate) = alternate_prefab {
        match ensure_runtime_prefab_parts(
            prims,
            mats,
            alternate,
            materials,
            material_specs,
            palette,
        ) {
            Ok(parts) => Some(parts),
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "authored-environment: alternate foliage prefab id='{}' source='{}' failed to load; primary-only fallback err='{}'",
                    alternate.id,
                    alternate.source,
                    error,
                );
                None
            }
        }
    } else {
        None
    };

    let placement_spec = effective_foliage_spec(foliage);
    let foliage_runtime =
        newengine_model_domain_api::FoliageInstanceRuntime::new(&placement_spec.settings, 0, 1);
    if placement_spec.max_count != foliage.max_count
        || placement_spec.grid_min != foliage.grid_min
        || placement_spec.grid_max != foliage.grid_max
        || (placement_spec.spacing - foliage.spacing).abs() > f32::EPSILON
    {
        newengine_ulog_api::ulog::info!(
            "authored-environment foliage stress override: authored_max_count={} effective_max_count={} grid={}..{} spacing={:.2} gate_threshold={:.2} policy='diagnostic-only; authored asset remains unchanged'",
            foliage.max_count,
            placement_spec.max_count,
            placement_spec.grid_min,
            placement_spec.grid_max,
            placement_spec.spacing,
            placement_spec.gate_threshold,
        );
    }

    let placements = collect_tree_placements(
        world,
        terrain,
        terrain_surface,
        &placement_spec,
        player_start,
    );
    let count = placements.len();
    if count == 0 {
        log_foliage_prefab_placement(
            &prefab.id,
            &prefab.source,
            &prefab.proxy,
            "static_baked_batch",
            runtime_parts.len(),
            0,
            placement_spec.max_count,
            placement_spec.grid_min,
            placement_spec.grid_max,
            placement_spec.spacing,
        );
        return;
    }

    // A single authored grid is partitioned between variants. This prevents double
    // density, coplanar overlap and duplicate collision bodies while keeping the
    // mixture deterministic for a fixed foliage seed.
    let mut primary_placements = Vec::with_capacity(count);
    let mut alternate_placements = Vec::with_capacity(count);
    let use_alternate = alternate_runtime_parts.is_some() && alternate_prefab.is_some();
    for placement in placements {
        let mut h = placement_spec.seed
            ^ (u64::from(placement.index).wrapping_mul(0x9e37_79b9_7f4a_7c15))
            ^ 0xd1b5_4a32_d192_ed03;
        h ^= h >> 30;
        h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 27;
        h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
        h ^= h >> 31;
        let variant_unit = ((h >> 40) as u32 as f32) / ((1u32 << 24) as f32);
        if use_alternate && variant_unit < placement_spec.alternate_weight {
            alternate_placements.push(placement);
        } else {
            primary_placements.push(placement);
        }
    }

    let instanced = matches!(
        foliage.render_options.role,
        newengine_model_domain_api::MeshRenderRole::FoliageInstanced
    );

    let mut total_render_entities = 0usize;
    if instanced {
        for &placement in &primary_placements {
            spawn_runtime_ydd_prefab_instance(
                world,
                &*prims,
                mats,
                root,
                &runtime_parts,
                placement,
                &foliage.render_options,
                &foliage_runtime,
            );
        }
        total_render_entities = total_render_entities
            .saturating_add(primary_placements.len().saturating_mul(runtime_parts.len()));

        if let (Some(alternate), Some(alternate_parts)) =
            (alternate_prefab, alternate_runtime_parts.as_ref())
        {
            for &placement in &alternate_placements {
                spawn_runtime_ydd_prefab_instance(
                    world,
                    &*prims,
                    mats,
                    root,
                    alternate_parts,
                    placement,
                    &foliage.render_options,
                    &foliage_runtime,
                );
            }
            total_render_entities = total_render_entities.saturating_add(
                alternate_placements
                    .len()
                    .saturating_mul(alternate_parts.len()),
            );
            newengine_ulog_api::ulog::info!(
                "authored-environment foliage variant mix: primary='{}' primary_placements={} alternate='{}' alternate_placements={} alternate_weight={:.3} total={} policy='single placement grid -> deterministic prefab partition'",
                prefab.id,
                primary_placements.len(),
                alternate.id,
                alternate_placements.len(),
                placement_spec.alternate_weight,
                count,
            );
        }

        newengine_ulog_api::ulog::info!(
            "authored-environment foliage instancing: primary='{}' placements={} primary_parts={} alternate='{}' alternate_placements={} alternate_parts={} ecs_instance_entities={} expected_gpu_batches={} policy='shared source geometry per variant + hardware instance buffers'",
            prefab.id,
            primary_placements.len(),
            runtime_parts.len(),
            alternate_prefab.map(|value| value.id.as_str()).unwrap_or(""),
            alternate_placements.len(),
            alternate_runtime_parts.as_ref().map(|parts| parts.len()).unwrap_or(0),
            total_render_entities,
            runtime_parts.len().saturating_add(
                alternate_runtime_parts.as_ref().map(|parts| parts.len()).unwrap_or(0)
            ),
        );
    } else {
        if !primary_placements.is_empty() {
            if let Err(error) = spawn_runtime_ydd_prefab_batch(
                world,
                prims,
                mats,
                root,
                prefab,
                &runtime_parts,
                &primary_placements,
                &foliage.render_options,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "authored-environment primary foliage batching failed prefab='{}' err='{}'",
                    prefab.id,
                    error,
                );
            }
        }
        if let (Some(alternate), Some(alternate_parts)) =
            (alternate_prefab, alternate_runtime_parts.as_ref())
        {
            if !alternate_placements.is_empty() {
                if let Err(error) = spawn_runtime_ydd_prefab_batch(
                    world,
                    prims,
                    mats,
                    root,
                    alternate,
                    alternate_parts,
                    &alternate_placements,
                    &foliage.render_options,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "authored-environment alternate foliage batching failed prefab='{}' err='{}'",
                        alternate.id,
                        error,
                    );
                }
            }
        }
    }

    let primary_collision_count =
        spawn_foliage_collision_proxies(world, root, &primary_placements, &placement_spec);
    let mut alternate_collision_count = 0usize;
    if !alternate_placements.is_empty() {
        let mut alternate_collision_spec = placement_spec.clone();
        alternate_collision_spec.collision_radius = placement_spec.alternate_collision_radius;
        alternate_collision_spec.collision_half_height =
            placement_spec.alternate_collision_half_height;
        alternate_collision_spec.collision_center = placement_spec.alternate_collision_center;
        alternate_collision_count = spawn_foliage_collision_proxies(
            world,
            root,
            &alternate_placements,
            &alternate_collision_spec,
        );
    }
    if placement_spec.collision_enabled
        && primary_collision_count.saturating_add(alternate_collision_count) != count
    {
        newengine_ulog_api::ulog::warn!(
            "authored-environment foliage collision proxy count mismatch placed={} colliders={} primary={} alternate={}",
            count,
            primary_collision_count.saturating_add(alternate_collision_count),
            primary_collision_count,
            alternate_collision_count,
        );
    }

    log_foliage_prefab_placement(
        &prefab.id,
        &prefab.source,
        &prefab.proxy,
        if instanced {
            "hardware_instanced"
        } else {
            "static_baked_batch"
        },
        runtime_parts.len(),
        primary_placements.len(),
        foliage.max_count,
        foliage.grid_min,
        foliage.grid_max,
        foliage.spacing,
    );
    if let (Some(alternate), Some(alternate_parts)) =
        (alternate_prefab, alternate_runtime_parts.as_ref())
    {
        log_foliage_prefab_placement(
            &alternate.id,
            &alternate.source,
            &alternate.proxy,
            if instanced {
                "hardware_instanced"
            } else {
                "static_baked_batch"
            },
            alternate_parts.len(),
            alternate_placements.len(),
            foliage.max_count,
            foliage.grid_min,
            foliage.grid_max,
            foliage.spacing,
        );
    }
}
