use core::f32::consts::PI;
use std::time::Duration;
use newengine_assets::AssetAccess;

fn read_u32_le(payload: &[u8], offset: &mut usize) -> Result<u32, String> {
    let end = offset.saturating_add(4);
    let bytes = payload
        .get(*offset..end)
        .ok_or_else(|| "NE3D payload truncated while reading u32".to_owned())?;
    *offset = end;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[inline]
fn read_f32_le(payload: &[u8], offset: &mut usize) -> Result<f32, String> {
    Ok(f32::from_bits(read_u32_le(payload, offset)?))
}

fn decode_ne3d_mesh(payload: &[u8]) -> Result<PrimitiveMesh, String> {
    if payload.len() < 20 || payload.get(0..4) != Some(b"NE3D") {
        return Err("invalid NE3D header".to_owned());
    }

    let mut offset = 4usize;
    let version = read_u32_le(payload, &mut offset)?;
    if version != 1 {
        return Err(format!("unsupported NE3D version={version}"));
    }

    let vertex_count = read_u32_le(payload, &mut offset)? as usize;
    let index_count = read_u32_le(payload, &mut offset)? as usize;
    let flags = read_u32_le(payload, &mut offset)?;

    if vertex_count == 0 || index_count == 0 {
        return Err(format!(
            "empty NE3D mesh vertices={vertex_count} indices={index_count}"
        ));
    }
    if vertex_count > 1_000_000 || index_count > 6_000_000 {
        return Err(format!(
            "NE3D mesh exceeds runtime limits vertices={vertex_count} indices={index_count}"
        ));
    }

    let has_normals = (flags & 0x1) != 0;
    let has_uvs = (flags & 0x2) != 0;

    let mut positions = Vec::with_capacity(vertex_count);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for _ in 0..vertex_count {
        let pos = [
            read_f32_le(payload, &mut offset)?,
            read_f32_le(payload, &mut offset)?,
            read_f32_le(payload, &mut offset)?,
        ];
        min.x = min.x.min(pos[0]);
        min.y = min.y.min(pos[1]);
        min.z = min.z.min(pos[2]);
        max.x = max.x.max(pos[0]);
        max.y = max.y.max(pos[1]);
        max.z = max.z.max(pos[2]);
        positions.push(pos);
    }

    let mut normals = Vec::with_capacity(vertex_count);
    if has_normals {
        for _ in 0..vertex_count {
            normals.push([
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
            ]);
        }
    } else {
        normals.resize(vertex_count, [0.0, 1.0, 0.0]);
    }

    let mut uvs = Vec::with_capacity(vertex_count);
    if has_uvs {
        for _ in 0..vertex_count {
            uvs.push([
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
            ]);
        }
    } else {
        uvs.resize(vertex_count, [0.0, 0.0]);
    }

    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        let index = read_u32_le(payload, &mut offset)?;
        if index as usize >= vertex_count {
            return Err(format!(
                "NE3D index out of bounds index={index} vertex_count={vertex_count}"
            ));
        }
        indices.push(index);
    }

    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(uvs)
        .map(|((pos, nrm), uv)| PrimitiveVertex { pos, nrm, uv })
        .collect::<Vec<_>>();

    let bounds_center = (min + max) * 0.5;
    let mut bounds_radius = 0.0f32;
    for v in &vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        bounds_radius = bounds_radius.max((p - bounds_center).length());
    }

    Ok(PrimitiveMesh {
        vertices,
        indices,
        bounds_center,
        bounds_radius: bounds_radius.max(0.001),
    })
}

fn load_ne3d_mesh_asset(logical_path: &str) -> Result<PrimitiveMesh, String> {
    let assets = AssetServiceClient::new(default_host_api());
    let id = assets
        .import_v1(logical_path)
        .map_err(|e| format!("asset.import_v1 failed path='{logical_path}' err='{e}'"))?;

    wait_ready(&assets, &id, Duration::from_secs(3))
        .map_err(|e| format!("asset not ready path='{logical_path}' id='{id}' err='{e:?}'"))?;

    let (meta, payload) = assets
        .blob_wire_v1(&id)
        .map_err(|e| format!("asset.blob_wire_v1 failed path='{logical_path}' id='{id}' err='{e}'"))?;

    if !meta.contains("kalitech.model3d.meta.v1") {
        log::warn!(
            "game-ready: geometry asset meta is not model3d schema path='{}' meta='{}'",
            logical_path,
            meta
        );
    }

    decode_ne3d_mesh(&payload)
}


fn build_procedural_skydome_mesh() -> PrimitiveMesh {
    const SLICES: u32 = 64;
    const STACKS: u32 = 32;
    let radius = 0.5_f32;
    let vert_w = SLICES + 1;
    let mut vertices = Vec::with_capacity(((SLICES + 1) * (STACKS + 1)) as usize);

    for y in 0..=STACKS {
        let v = y as f32 / STACKS as f32;
        let phi = v * PI;
        let (sp, cp) = phi.sin_cos();
        for x in 0..=SLICES {
            let u = x as f32 / SLICES as f32;
            let theta = u * 2.0 * PI;
            let (st, ct) = theta.sin_cos();
            let outward = Vec3::new(ct * sp, cp, st * sp);
            let p = outward * radius;
            let inward = -outward;
            vertices.push(PrimitiveVertex {
                pos: [p.x, p.y, p.z],
                nrm: [inward.x, inward.y, inward.z],
                uv: [u, 1.0 - v],
            });
        }
    }

    let mut indices = Vec::with_capacity((SLICES * STACKS * 6) as usize);
    for y in 0..STACKS {
        for x in 0..SLICES {
            let i0 = y * vert_w + x;
            let i1 = i0 + 1;
            let i2 = i0 + vert_w;
            let i3 = i2 + 1;
            // Reversed winding: the dome is viewed from inside.
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
        }
    }

    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: radius,
    }
}

fn ensure_skydome_primitive(prims: &mut PrimitiveRegistry, logical_path: &str) -> Option<PrimitiveId> {
    if prims.is_registered(SKYDOME_PRIMITIVE_ID) {
        return Some(SKYDOME_PRIMITIVE_ID);
    }

    if logical_path.eq_ignore_ascii_case("procedural:skydome") {
        let mesh = build_procedural_skydome_mesh();
        let vertex_count = mesh.vertices.len();
        let index_count = mesh.indices.len();
        prims.register_mesh(
            SKYDOME_PRIMITIVE_ID,
            "Procedural/SkyDome".to_owned(),
            mesh,
        );
        log::info!(
            "game-ready: procedural skydome selected vertices={} indices={}",
            vertex_count,
            index_count
        );
        return Some(SKYDOME_PRIMITIVE_ID);
    }

    match load_ne3d_mesh_asset(logical_path) {
        Ok(mesh) => {
            let vertex_count = mesh.vertices.len();
            let index_count = mesh.indices.len();
            prims.register_mesh(
                SKYDOME_PRIMITIVE_ID,
                format!("Imported/SkyDome ({logical_path})"),
                mesh,
            );
            log::info!(
                "game-ready: skydome imported through AssetManager/geometryImporter path='{}' vertices={} indices={}",
                logical_path,
                vertex_count,
                index_count
            );
            Some(SKYDOME_PRIMITIVE_ID)
        }
        Err(e) => {
            let mesh = build_procedural_skydome_mesh();
            let vertex_count = mesh.vertices.len();
            let index_count = mesh.indices.len();
            prims.register_mesh(
                SKYDOME_PRIMITIVE_ID,
                format!("Procedural/SkyDome fallback ({logical_path})"),
                mesh,
            );
            log::warn!(
                "game-ready: skydome mesh import failed path='{}' err='{}'; using procedural UV dome fallback vertices={} indices={}",
                logical_path,
                e,
                vertex_count,
                index_count
            );
            Some(SKYDOME_PRIMITIVE_ID)
        }
    }
}

fn spawn_skydome(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    materials: DemoMaterials,
    root: EntityId,
    spec: &GameReadySkySpec,
    color: [f32; 4],
) {
    let Some(primitive_id) = ensure_skydome_primitive(prims, &spec.mesh) else {
        return;
    };

    let sky = spawn_game_primitive(
        world,
        &*prims,
        mats,
        PrimitiveSpawnSpec {
            parent: root,
            primitive_id,
            material_id: materials.sky,
            name: "Sky/Imported-SkyDome",
            position: Vec3::ZERO,
            scale: Vec3::splat(spec.radius),
            color,
        },
    );
    // Sky is a background primitive, not world geometry. Keeping its massive
    // bounds in the scene union makes directional shadow fitting unstable and
    // lets shadow-map UVs appear as dark projection bands on the dome.
    let _ = world.remove::<newengine_bounds::Bounds>(sky);
    let _ = apply_exact_material(world, mats, sky, materials.sky, materials.sky, color);
}

fn to_fps_demo_rules(spec: &GameReadyGameplaySpec) -> FpsDemoRules {
    FpsDemoRules {
        default_status: spec.default_status.clone(),
        pickup_status: spec.pickup_status.clone(),
        hazard_status: spec.hazard_status.clone(),
        goal_locked_status: spec.goal_locked_status.clone(),
        goal_complete_status: spec.goal_complete_status.clone(),
        failed_progress_label: spec.failed_progress_label.clone(),
        completed_progress_label: spec.completed_progress_label.clone(),
        player: FpsPlayerTuning {
            body_radius: spec.player_collision.radius,
            body_half_height: spec.player_collision.half_height,
            visual_radius: spec.player_visual.radius,
            visual_half_height: spec.player_visual.half_height,
            camera_eye_height: spec.player_visual.camera_eye_height,
            sprint_multiplier: spec.player_visual.sprint_multiplier,
            gravity: spec.physics.gravity,
            contact_skin: spec.physics.contact_skin,
        }
        .sanitized(),
    }
}

pub(super) fn bootstrap_fps_game_ready_scene(
    scene: &mut Scene,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) -> Option<EntityId> {
    *scene = Scene::new();
    bootstrap_runtime_scene(scene);

    let root = ensure_root(scene);
    let active_camera = scene.active_camera();
    let map = load_game_ready_map_profile();
    let materials = register_demo_materials(mats, &map.palette, &map.materials);
    let world = scene.world_mut();

    let rules = to_fps_demo_rules(&map.gameplay);
    world.insert_resource(rules.clone());
    world.insert_resource(FpsDemoState::from_rules(
        0,
        map.title.clone(),
        map.objective.clone(),
        &rules,
    ));
    world.insert_resource(GameReadyWorldLaunchGate::new(
        "waiting for CPU scene assembly and GPU material residency",
    ));

    configure_game_ready_lighting(world, &map.lighting);

    let terrain = spawn_procedural_terrain(world, mats, root, materials.terrain, &map.terrain, map.palette.terrain);
    spawn_terrain_collision_tiles(world, root, terrain, &map.terrain);
    spawn_foliage_prefabs(
        world,
        prims,
        mats,
        root,
        terrain,
        materials,
        &map.palette,
        &map.foliage,
        &map.prefabs,
        map.player.start,
    );
    spawn_skydome(world, prims, mats, materials, root, &map.sky, map.palette.sky);

    let start_x = map.player.start.x;
    let start_z = map.player.start.z;
    let player_tuning = rules.player.sanitized();
    let start_y = terrain_height(world, terrain, start_x, start_z)
        + player_tuning.body_half_height
        + player_tuning.body_radius
        + player_tuning.contact_skin;
    let player = spawn_default_player_with_tuning(
        world,
        Some(root),
        "Player/FPS",
        Vec3::new(start_x, start_y, start_z),
        player_tuning,
    );
    let _ = world.insert(player, DisplayVisibility { mode: DisplayMode::EditorOnly });
    if let Some(motor) = world.get_mut::<newengine_sim::CharacterMotor>(player) {
        motor.move_speed = map.player.move_speed;
        motor.look_sens = map.player.look_sens;
        motor.yaw = map.player.yaw;
    }
    if let Some(t) = world.get_mut_tracked::<Transform>(player) {
        t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
    }

    if let Some(cam) = active_camera {
        if let Some(t) = world.get_mut_tracked::<Transform>(cam) {
            t.position = Vec3::new(start_x, start_y + player_tuning.camera_eye_height, start_z);
            t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
        }
    }

    let _ = scene.validate_invariants();
    Some(player)
}
