use super::super::paths::profile_asset_candidates;
use super::*;

use newengine_assets::{AssetDecodeRequest, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_authored_xml as authored_xml;

use super::ymap_read_diagnostics::{
    format_asset_roots, log_loaded_profile_summary, log_ymap_value_summary,
};

#[derive(Debug, Deserialize)]
pub(super) struct RawShadowSpec {
    #[serde(default = "default_shadow_enabled")]
    pub(super) enabled: bool,
    #[serde(default = "default_shadow_resolution")]
    pub(super) resolution: u32,
    #[serde(default = "default_shadow_cascade_count")]
    pub(super) cascade_count: u32,
    #[serde(default = "default_shadow_max_distance")]
    pub(super) max_distance: f32,
    #[serde(default = "default_shadow_softness")]
    pub(super) softness: f32,
    #[serde(default = "default_shadow_bias")]
    pub(super) bias: f32,
    #[serde(default = "default_shadow_normal_bias")]
    pub(super) normal_bias: f32,
    #[serde(default = "default_shadow_contact_strength")]
    pub(super) contact_strength: f32,
}

impl Default for RawShadowSpec {
    fn default() -> Self {
        Self {
            enabled: default_shadow_enabled(),
            resolution: default_shadow_resolution(),
            cascade_count: default_shadow_cascade_count(),
            max_distance: default_shadow_max_distance(),
            softness: default_shadow_softness(),
            bias: default_shadow_bias(),
            normal_bias: default_shadow_normal_bias(),
            contact_strength: default_shadow_contact_strength(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RawFoliageSpec {
    #[serde(default)]
    pub(super) enabled: bool,
    #[serde(default = "default_foliage_prefab")]
    pub(super) prefab: String,
    #[serde(default = "default_foliage_seed")]
    pub(super) seed: u64,
    #[serde(default = "default_foliage_grid_min")]
    pub(super) grid_min: i32,
    #[serde(default = "default_foliage_grid_max")]
    pub(super) grid_max: i32,
    #[serde(default = "default_foliage_spacing")]
    pub(super) spacing: f32,
    #[serde(default = "default_foliage_jitter")]
    pub(super) jitter: f32,
    #[serde(default = "default_foliage_gate_threshold")]
    pub(super) gate_threshold: f32,
    #[serde(default)]
    pub(super) max_count: u32,
    #[serde(default = "default_foliage_min_scale")]
    pub(super) min_scale: f32,
    #[serde(default = "default_foliage_max_scale")]
    pub(super) max_scale: f32,
    #[serde(default = "default_foliage_min_player_distance")]
    pub(super) min_player_distance: f32,
    #[serde(default = "default_foliage_edge_margin")]
    pub(super) edge_margin: f32,
    #[serde(default = "default_foliage_surface_offset")]
    pub(super) surface_offset: f32,
}

impl Default for RawFoliageSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            prefab: default_foliage_prefab(),
            seed: default_foliage_seed(),
            grid_min: default_foliage_grid_min(),
            grid_max: default_foliage_grid_max(),
            spacing: default_foliage_spacing(),
            jitter: default_foliage_jitter(),
            gate_threshold: default_foliage_gate_threshold(),
            max_count: 0,
            min_scale: default_foliage_min_scale(),
            max_scale: default_foliage_max_scale(),
            min_player_distance: default_foliage_min_player_distance(),
            edge_margin: default_foliage_edge_margin(),
            surface_offset: default_foliage_surface_offset(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct RawPrefabSpec {
    #[serde(default)]
    pub(super) id: String,
    #[serde(default)]
    pub(super) source: String,
    #[serde(default = "default_prefab_proxy")]
    pub(super) proxy: String,
    /// Optional exact NEMAT selector used by static world geometry.
    #[serde(default)]
    pub(super) material: String,
    #[serde(default = "default_prefab_enabled")]
    pub(super) enabled: bool,
    #[serde(default)]
    pub(super) position: [f32; 3],
    #[serde(default)]
    pub(super) rotation_ypr: [f32; 3],
    #[serde(default = "default_definition_scale")]
    pub(super) scale: [f32; 3],
}

#[derive(Debug, Deserialize)]
pub(super) struct RawDefinitionInstanceSpec {
    #[serde(default)]
    pub(super) definition_ref: String,
    /// Declarative apply behavior for this `.ymap` definition placement.
    /// Default is metadata-only so `.ytyp` dependencies remain graph inputs,
    /// not implicit render/spawn commands.
    #[serde(default = "default_definition_apply_mode")]
    pub(super) apply_mode: String,
    #[serde(default)]
    pub(super) position: [f32; 3],
    #[serde(default)]
    pub(super) rotation_ypr: [f32; 3],
    #[serde(default = "default_definition_scale")]
    pub(super) scale: [f32; 3],
}

pub(in crate::scene_bridge::game_ready) fn load_game_ready_map_profile(
) -> Result<GameReadyMapProfile, Vec<String>> {
    load_profile_from_asset_manager()
}

fn load_profile_from_asset_manager() -> Result<GameReadyMapProfile, Vec<String>> {
    use newengine_assets::AssetService;

    if !newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "game-ready: AssetManager service '{}' unavailable while resolving authored map",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        );
        return Err(vec![format!(
            "AssetManager service '{}' unavailable while resolving authored map",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        )]);
    }

    let assets =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let roots = newengine_runtime_host::asset_bootstrap::collect_app_asset_roots(
        "active-profile",
        "NEWENGINE_GAME_ASSETS_DIR",
    );
    newengine_runtime_host::asset_bootstrap::mount_asset_roots_best_effort(&assets, &roots);

    let candidates = profile_asset_candidates();
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: begin gateway='{}' candidates={} asset_roots={} roots=[{}] policy='AssetManager decode_v1 only'",
        newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
        candidates.len(),
        roots.len(),
        format_asset_roots(&roots),
    );

    let mut errors = Vec::new();
    for (index, logical_path) in candidates.into_iter().enumerate() {
        newengine_ulog_api::ulog::info!(
            "game-ready ymap read: candidate begin index={} path='{}'",
            index,
            logical_path,
        );
        match load_profile_asset(&assets, &logical_path) {
            Ok(profile) => {
                let trace = assets
                    .resolve_trace_json_v1(&logical_path)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));
                newengine_ulog_api::ulog::info!(
                    "game-ready ymap read: candidate selected index={} path='{}' trace={}",
                    index,
                    logical_path,
                    trace,
                );
                newengine_ulog_api::ulog::info!(
                    "game-ready: loaded authored map asset='{}'",
                    logical_path,
                );
                return Ok(profile);
            }
            Err(e) => {
                let trace = assets
                    .resolve_trace_json_v1(&logical_path)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));
                let message = format!("path='{logical_path}' err='{e}' trace={trace}");
                newengine_ulog_api::ulog::info!(
                    "game-ready ymap read: candidate rejected index={} {}",
                    index,
                    message
                );
                errors.push(message);
            }
        }
    }

    Err(errors)
}

fn load_profile_asset(
    assets: &newengine_assets::AssetServiceClient,
    logical_path: &str,
) -> Result<GameReadyMapProfile, String> {
    if !logical_path
        .to_ascii_lowercase()
        .split('@')
        .next()
        .unwrap_or(logical_path)
        .ends_with(&format!(
            ".{}",
            newengine_asset_format_nef8::ymap::EXTENSION
        ))
    {
        return Err(format!(
            "non-canonical authored map rejected path='{logical_path}' expected='.{}' policy='authored maps are NEF8/ListFile, not runtime plain JSON'", newengine_asset_format_nef8::ymap::EXTENSION
        ));
    }

    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: canonical accepted path='{}' extension='{}'",
        logical_path,
        newengine_asset_format_nef8::ymap::EXTENSION,
    );

    let output_kind = ASSET_LIST_FILE_BODY_OUTPUT;
    let request = AssetDecodeRequest {
        logical_path: logical_path.to_owned(),
        output_kind: output_kind.to_owned(),
        selector: serde_json::Value::Null,
    };
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: decode start path='{}' output='{}' selector=null",
        logical_path,
        output_kind,
    );
    let payload = assets.decode_v1(&request).map_err(|e| {
        format!("asset.decode_v1 failed path='{logical_path}' output='{output_kind}' err='{e}'")
    })?;
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: decode complete path='{}' output='{}' payload_bytes={}",
        logical_path,
        output_kind,
        payload.len(),
    );
    if !authored_xml::body_is_xml(&payload) {
        return Err(format!(
            "ymap body must be XML path='{logical_path}' output='{output_kind}' payload_bytes={} policy='authored map metadata uses XML presentation inside NEF8; JSON runtime map bodies are forbidden'",
            payload.len()
        ));
    }
    let value = parse_ymap_xml_payload(&payload, logical_path)?;
    log_ymap_value_summary(logical_path, &value);
    newengine_ulog_api::ulog::info!(
        "game-ready: decoded authored .ymap path='{}' output='{}' policy='NEF8/ListFile body from engine.assets; XML map semantics stay outside AssetManager'",
        logical_path,
        output_kind
    );
    parse_map_definition_payload(value, logical_path)
}

fn parse_ymap_xml_payload(payload: &[u8], logical_path: &str) -> Result<serde_json::Value, String> {
    let text = std::str::from_utf8(payload)
        .map_err(|e| format!("ymap XML body is not UTF-8 path='{logical_path}' err='{e}'"))?;
    let doc = authored_xml::parse_xml_document(text, &format!("ymap path='{logical_path}'"))?;
    let root = doc.root_element();
    if !root.has_tag_name("YmapMapDefinition") && !root.has_tag_name("MapDefinition") {
        return Err(format!(
            "ymap XML root must be <YmapMapDefinition> path='{logical_path}' actual='{}'",
            root.tag_name().name()
        ));
    }
    let schema = root.attribute("schema").unwrap_or_default();
    if !schema.starts_with("newengine.map.definition.") {
        return Err(format!("ymap unsupported XML schema path='{logical_path}' schema='{schema}' expected='newengine.map.definition.*'"));
    }
    let child_elements = root.children().filter(|child| child.is_element()).count();
    newengine_ulog_api::ulog::info!(
        "game-ready ymap read: XML accepted path='{}' payload_bytes={} root='{}' schema='{}' child_elements={}",
        logical_path,
        payload.len(),
        root.tag_name().name(),
        schema,
        child_elements,
    );

    let mut root_json = serde_json::Map::new();
    root_json.insert(
        "schema".to_owned(),
        serde_json::Value::String(schema.to_owned()),
    );
    if let Some(map_node) =
        authored_xml::xml_child(root, "map").or_else(|| authored_xml::xml_child(root, "Map"))
    {
        root_json.insert("map".to_owned(), ymap_node_object(map_node));
    } else if let Some(profile_node) = authored_xml::xml_child(root, "profile")
        .or_else(|| authored_xml::xml_child(root, "Profile"))
    {
        root_json.insert("profile".to_owned(), ymap_node_object(profile_node));
    } else {
        return Err(format!(
            "ymap XML has no <map> or <profile> node path='{logical_path}'"
        ));
    }
    Ok(serde_json::Value::Object(root_json))
}

fn parse_map_definition_payload(
    value: serde_json::Value,
    logical_path: &str,
) -> Result<GameReadyMapProfile, String> {
    let schema = value
        .get("schema")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !schema.is_empty() && !schema.starts_with("newengine.map.definition.") {
        return Err(format!(
            "ymap unsupported schema path='{logical_path}' schema='{schema}' expected='newengine.map.definition.*'"
        ));
    }

    if let Some(profile) = value.pointer("/map/profile").cloned() {
        return parse_payload(profile, "ymap.map.profile", logical_path);
    }
    if let Some(profile) = value.get("profile").cloned() {
        return parse_payload(profile, "ymap.profile", logical_path);
    }
    if value.get("scene").is_some() {
        return Err(format!(
            "ymap scene payload rejected path='{logical_path}' policy='use newengine.map.definition.* with map.profile / profile / payload'"
        ));
    }
    if let Some(payload) = value.get("payload").cloned() {
        return parse_payload(payload, "ymap.payload", logical_path);
    }
    parse_payload(value, "ymap.root", logical_path)
}

fn parse_payload(
    value: serde_json::Value,
    source_label: &str,
    logical_path: &str,
) -> Result<GameReadyMapProfile, String> {
    let raw: RawGameReadyPayload = serde_json::from_value(value)
        .map_err(|e| format!("map payload parse failed source='{source_label}': {e}"))?;
    let profile = raw.into_profile();
    log_loaded_profile_summary(logical_path, source_label, &profile);
    Ok(profile)
}

fn ymap_node_object(node: authored_xml::XmlNode<'_, '_>) -> serde_json::Value {
    let tag = node.tag_name().name();
    if tag.eq_ignore_ascii_case("definition_refs") {
        let refs = node
            .children()
            .filter(|child| child.is_element())
            .filter_map(|child| {
                child
                    .attribute("value")
                    .or_else(|| child.attribute("ref"))
                    .map(str::trim)
            })
            .filter(|value| !value.is_empty())
            .map(|value| serde_json::Value::String(value.to_owned()))
            .collect::<Vec<_>>();
        return serde_json::Value::Array(refs);
    }
    if tag.eq_ignore_ascii_case("definitions")
        || tag.eq_ignore_ascii_case("placements")
        || tag.eq_ignore_ascii_case("prefabs")
        || tag.eq_ignore_ascii_case("policy")
        || tag.eq_ignore_ascii_case("layers")
        || tag.eq_ignore_ascii_case("surface_layers")
        || tag.eq_ignore_ascii_case("pickups")
        || tag.eq_ignore_ascii_case("targets")
        || tag.eq_ignore_ascii_case("hazards")
        || tag.eq_ignore_ascii_case("goals")
    {
        let items = node
            .children()
            .filter(|child| child.is_element())
            .map(ymap_node_object)
            .collect::<Vec<_>>();
        return serde_json::Value::Array(items);
    }
    if tag.eq_ignore_ascii_case("DefinitionRef")
        || tag.eq_ignore_ascii_case("Policy")
        || tag.eq_ignore_ascii_case("Item")
    {
        if let Some(value) = node.attribute("value").or_else(|| node.attribute("ref")) {
            return authored_xml::xml_scalar(value);
        }
    }

    let element_children = node.children().filter(|child| child.is_element()).count();
    if element_children == 0 {
        let attr_count = node.attributes().count();
        if attr_count == 1 {
            if let Some(value) = node.attribute("value") {
                return authored_xml::xml_scalar(value);
            }
        }
    }

    let mut map = serde_json::Map::new();
    for attr in node.attributes() {
        map.insert(
            attr.name().to_owned(),
            authored_xml::xml_scalar(attr.value()),
        );
    }
    for child in node.children().filter(|child| child.is_element()) {
        let key = child.tag_name().name();
        let value = ymap_node_object(child);
        ymap_insert_child(&mut map, key, value);
    }
    if map.is_empty() {
        if let Some(text) = node.text().map(str::trim).filter(|text| !text.is_empty()) {
            return authored_xml::xml_scalar(text);
        }
    }
    serde_json::Value::Object(map)
}

fn ymap_insert_child(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: serde_json::Value,
) {
    let key = match key {
        "Definition" => "definitions",
        "Placement" => "placements",
        "Prefab" => "prefabs",
        "Layer" | "SurfaceLayer" => "layers",
        "Pickup" => "pickups",
        "Target" => "targets",
        "Hazard" => "hazards",
        "Goal" => "goals",
        other => other,
    };
    match map.get_mut(key) {
        Some(serde_json::Value::Array(items)) => items.push(value),
        Some(existing) => {
            let old = std::mem::replace(existing, serde_json::Value::Null);
            *existing = serde_json::Value::Array(vec![old, value]);
        }
        None => {
            map.insert(key.to_owned(), value);
        }
    }
}

impl RawGameReadyPayload {
    fn into_profile(self) -> GameReadyMapProfile {
        let terrain_chunk_radius = self.terrain.streaming.chunk_radius.clamp(
            0,
            newengine_scene::SceneStreamingBudget::MAX_RESIDENT_RADIUS,
        );
        let terrain_unload_radius = self.terrain.streaming.unload_radius.clamp(
            (terrain_chunk_radius + 1).max(1),
            newengine_scene::SceneStreamingBudget::MAX_UNLOAD_RADIUS,
        );

        GameReadyMapProfile {
            title: self.title,
            objective: self.objective,
            player: GameReadyPlayerSpec {
                start: arr3(self.player.start),
                yaw: self.player.yaw,
                move_speed: self.player.move_speed,
                look_sens: self.player.look_sens,
                model: GameReadyPlayerModelSpec {
                    enabled: self.player.model.enabled
                        && !self.player.model.source.trim().is_empty(),
                    source: if self.player.model.enabled {
                        non_empty_or(self.player.model.source, default_player_model_source())
                    } else {
                        String::new()
                    },
                    properties_ref: sanitize_asset_path(self.player.model.properties_ref),
                    texture_dictionary: sanitize_texture_path(self.player.model.texture_dictionary),
                    skeleton: sanitize_asset_path(self.player.model.skeleton),
                    target_height: self.player.model.target_height.clamp(0.25, 3.0),
                    eye_height_ratio: self.player.model.eye_height_ratio.clamp(0.55, 0.98),
                    local_offset: arr3(self.player.model.local_offset),
                    yaw_offset: self.player.model.yaw_offset,
                    hide_in_first_person: self.player.model.hide_in_first_person,
                    render_options: newengine_model_domain_api::MeshRenderOptions::character_body(),
                },
            },
            terrain: GameReadyTerrainSpec {
                enabled: self.terrain.enabled,
                seed: self.terrain.seed,
                cells_x: self.terrain.cells_x.clamp(16, 80),
                cells_z: self.terrain.cells_z.clamp(16, 80),
                size_x: self.terrain.size_x.max(4.0),
                size_z: self.terrain.size_z.max(4.0),
                base_height: self.terrain.base_height,
                height_scale: self.terrain.height_scale.clamp(0.05, 1.45),
                render_options: newengine_model_domain_api::MeshRenderOptions::terrain_patch(),
                generator: GameReadyTerrainGeneratorSpec {
                    id: self.terrain.generator.id,
                    ridged_seed_xor: self.terrain.generator.ridged_seed_xor,
                    ridged_frequency: self.terrain.generator.ridged_frequency.max(0.001),
                    ridged_amplitude: self.terrain.generator.ridged_amplitude,
                    ridged_shape_edge0: self.terrain.generator.ridged_shape_edge0,
                    ridged_shape_edge1: self.terrain.generator.ridged_shape_edge1,
                    veins_seed_xor: self.terrain.generator.veins_seed_xor,
                    veins_frequency: self.terrain.generator.veins_frequency.max(0.001),
                    veins_amplitude: self.terrain.generator.veins_amplitude,
                    smoothing_passes: self.terrain.generator.smoothing_passes.min(16),
                    smoothing_strength: self.terrain.generator.smoothing_strength.clamp(0.0, 1.0),
                },
                surface: sanitize_terrain_surface_spec(self.terrain.surface),
                heightmap: sanitize_terrain_heightmap_spec(self.terrain.heightmap),
                streaming: GameReadyTerrainStreamingSpec {
                    enabled: self.terrain.streaming.enabled,
                    chunk_radius: terrain_chunk_radius,
                    unload_radius: terrain_unload_radius,
                    max_chunks_per_frame: self.terrain.streaming.max_chunks_per_frame.clamp(
                        1,
                        newengine_scene::SceneStreamingBudget::MAX_COMMITS_PER_TICK,
                    ),
                },
            },
            sky: GameReadySkySpec {
                definition_ref: non_empty_or(self.sky.definition_ref, default_sky_definition_ref()),
                render_options: newengine_model_domain_api::MeshRenderOptions::sky_background(),
                radius: self.sky.radius.max(16.0),
                mesh: non_empty_or(self.sky.mesh, default_skydome_mesh()),
                follow_camera: self.sky.follow_camera,
                cloud_dictionary: non_empty_or(
                    self.sky.cloud_dictionary,
                    default_cloud_dictionary(),
                ),
                cloud_profile: non_empty_or(self.sky.cloud_profile, default_cloud_profile()),
                sun_radius: self.sky.sun_radius.clamp(1.0, 64.0),
                moon_radius: self.sky.moon_radius.clamp(1.0, 64.0),
                moon_texture: non_empty_or(self.sky.moon_texture, default_moon_texture()),
                atmosphere: sanitize_sky_atmosphere_spec(self.sky.atmosphere),
            },
            materials: GameReadyMaterialSetSpec {
                terrain: sanitize_material_spec_with_default_asset(
                    self.materials.terrain,
                    default_terrain_material(),
                ),
                sky: sanitize_material_spec_with_default_asset(
                    self.materials.sky,
                    default_sky_material(),
                ),
                sun: sanitize_material_spec_with_default_asset(
                    self.materials.sun,
                    default_sun_material(),
                ),
                moon: sanitize_material_spec_with_default_asset(
                    self.materials.moon,
                    default_moon_material(),
                ),
                tree_bark: sanitize_material_spec_with_default_asset(
                    self.materials.tree_bark,
                    default_tree_bark_material(),
                ),
                tree_leaf: sanitize_material_spec_with_default_asset(
                    self.materials.tree_leaf,
                    default_tree_leaf_material(),
                ),
                tree_branch: sanitize_material_spec_with_default_asset(
                    self.materials.tree_branch,
                    default_tree_branch_material(),
                ),
            },
            lighting: sanitize_lighting_spec(self.lighting),
            foliage: sanitize_foliage_spec(self.foliage),
            prefabs: self
                .prefabs
                .into_iter()
                .filter_map(sanitize_prefab_spec)
                .collect(),
            definitions: self
                .definitions
                .into_iter()
                .filter_map(sanitize_definition_instance_spec)
                .collect(),
            gameplay: GameReadyGameplaySpec {
                default_status: non_empty_or(self.gameplay.default_status, default_status_text()),
                pickup_status: non_empty_or(self.gameplay.pickup_status, default_pickup_status()),
                target_status: non_empty_or(self.gameplay.target_status, default_target_status()),
                hazard_status: non_empty_or(self.gameplay.hazard_status, default_hazard_status()),
                goal_locked_status: non_empty_or(
                    self.gameplay.goal_locked_status,
                    default_goal_locked_status(),
                ),
                goal_complete_status: non_empty_or(
                    self.gameplay.goal_complete_status,
                    default_goal_complete_status(),
                ),
                failed_progress_label: non_empty_or(
                    self.gameplay.failed_progress_label,
                    default_failed_progress_label(),
                ),
                completed_progress_label: non_empty_or(
                    self.gameplay.completed_progress_label,
                    default_completed_progress_label(),
                ),
                player_collision: GameReadyPlayerCollisionSpec {
                    radius: self.gameplay.player_collision.radius.clamp(0.05, 5.0),
                    half_height: self.gameplay.player_collision.half_height.clamp(0.05, 8.0),
                },
                player_visual: GameReadyPlayerVisualSpec {
                    radius: self.gameplay.player_visual.radius.clamp(0.05, 8.0),
                    half_height: self.gameplay.player_visual.half_height.clamp(0.05, 12.0),
                    camera_eye_height: self
                        .gameplay
                        .player_visual
                        .camera_eye_height
                        .clamp(0.05, 12.0),
                    sprint_multiplier: self
                        .gameplay
                        .player_visual
                        .sprint_multiplier
                        .clamp(1.0, 8.0),
                },
                physics: GameReadyPhysicsSpec {
                    gravity: self.gameplay.physics.gravity.clamp(0.0, 80.0),
                    contact_skin: self.gameplay.physics.contact_skin.clamp(0.0, 0.50),
                },
                mission: GameReadyMissionSpec {
                    pickups: self
                        .gameplay
                        .mission
                        .pickups
                        .into_iter()
                        .filter_map(sanitize_mission_pickup_spec)
                        .collect(),
                    targets: self
                        .gameplay
                        .mission
                        .targets
                        .into_iter()
                        .filter_map(sanitize_mission_target_spec)
                        .collect(),
                    hazards: self
                        .gameplay
                        .mission
                        .hazards
                        .into_iter()
                        .filter_map(sanitize_mission_hazard_spec)
                        .collect(),
                    goals: self
                        .gameplay
                        .mission
                        .goals
                        .into_iter()
                        .filter_map(sanitize_mission_goal_spec)
                        .collect(),
                },
            },
            palette: GameReadyPaletteSpec {
                terrain: self.palette.terrain,
                sky: self.palette.sky,
                sky_emissive: self.palette.sky_emissive,
                tree_bark: self.palette.tree_bark,
                tree_leaf: self.palette.tree_leaf,
                tree_branch: self.palette.tree_branch,
            },
        }
    }
}
