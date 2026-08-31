use super::*;
use parking_lot::Mutex;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) type DefinitionCache = Arc<Mutex<BTreeMap<String, ResolvedMapDefinitionEntry>>>;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionRefs {
    drawable_refs: Vec<String>,
    material_refs: Vec<String>,
    collision_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
struct ResolvedMapDefinitionModelExplanation {
    collision_policy: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct ResolvedMapDefinitionEntry {
    refs: ResolvedMapDefinitionRefs,
    semantic_tags: Vec<String>,
    model_explanation: ResolvedMapDefinitionModelExplanation,
    arbitrary_metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

fn load_cell(
    map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
) -> Result<newengine_assets_api::MapResolvedCellV2, String> {
    let request = serde_json::to_vec(&newengine_assets_api::MapCellRequestV1 {
        map_ref: map_ref.to_owned(),
        coord,
    })
    .map_err(|error| format!("map cell request encode failed: {error}"))?;
    let bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
        newengine_assets_api::maps_method::CELL_V2,
        &request,
    )
    .map_err(|error| {
        format!(
            "map cell request failed map='{map_ref}' cell={},{} err='{error}'",
            coord.x, coord.z
        )
    })?
    .ok_or_else(|| {
        format!(
            "engine.assets.maps unavailable map='{map_ref}' cell={},{}",
            coord.x, coord.z
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "invalid MapResolvedCellV2 map='{map_ref}' cell={},{} err='{error}'",
            coord.x, coord.z
        )
    })
}

fn resolve_definition(
    cache: &DefinitionCache,
    definition_ref: &str,
) -> Result<ResolvedMapDefinitionEntry, String> {
    if let Some(existing) = cache.lock().get(definition_ref).cloned() {
        return Ok(existing);
    }
    let payload = serde_json::to_vec(&serde_json::json!({ "definition_ref": definition_ref }))
        .map_err(|error| format!("definition request encode failed: {error}"))?;
    let bytes = newengine_core::call_service_v1_optional(
        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets_api::definitions_method::ENTRY_JSON_V1,
        &payload,
    )
    .map_err(|error| {
        format!("definition request failed definition_ref='{definition_ref}' err='{error}'")
    })?
    .ok_or_else(|| {
        format!("engine.assets.definitions unavailable definition_ref='{definition_ref}'")
    })?;
    let parsed: ResolvedMapDefinitionEntry = serde_json::from_slice(&bytes).map_err(|error| {
        format!("invalid definition DTO definition_ref='{definition_ref}' err='{error}'")
    })?;
    let mut locked = cache.lock();
    Ok(locked
        .entry(definition_ref.to_owned())
        .or_insert_with(|| parsed.clone())
        .clone())
}

#[derive(Clone, Debug, Default)]
struct DefinitionSurfaceBinding {
    id: String,
    events: std::collections::BTreeMap<String, String>,
    ground_placement_surface: bool,
}

fn definition_surface_binding(definition: &ResolvedMapDefinitionEntry) -> DefinitionSurfaceBinding {
    fn parse(value: &serde_json::Value) -> Option<DefinitionSurfaceBinding> {
        let object = value.as_object()?;
        let id = object
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let events = object
            .get("events")
            .and_then(serde_json::Value::as_object)
            .into_iter()
            .flat_map(|events| events.iter())
            .filter_map(|(signal, event_id)| {
                let signal = signal.trim().to_owned();
                let event_id = event_id.as_str()?.trim().to_owned();
                (!signal.is_empty() && !event_id.is_empty()).then_some((signal, event_id))
            })
            .collect();
        let ground_placement_surface = object
            .get("ground_placement_surface")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        Some(DefinitionSurfaceBinding {
            id,
            events,
            ground_placement_surface,
        })
    }

    for root_name in ["metadata", "namespaces"] {
        let Some(root) = definition
            .arbitrary_metadata
            .get(root_name)
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        for namespace in ["newengine.physics.surface", "engine.physics.surface"] {
            if let Some(binding) = root.get(namespace).and_then(parse) {
                return binding;
            }
        }
    }
    DefinitionSurfaceBinding::default()
}

fn placement_is_spawn(placement: &newengine_assets_api::MapPlacementV1) -> bool {
    placement.tags.iter().any(|tag| {
        matches!(
            tag.trim().to_ascii_lowercase().as_str(),
            "player_spawn" | "info_player_start" | "spawn.player"
        )
    }) || matches!(
        placement.apply_mode.trim().to_ascii_lowercase().as_str(),
        "player_spawn" | "info_player_start"
    )
}

fn cell_prefabs(
    logical_map_ref: &str,
    resolved: &newengine_assets_api::MapResolvedCellV2,
    definition_cache: &DefinitionCache,
) -> Result<
    (
        Vec<GameReadyPrefabSpec>,
        Vec<GameReadyPrefabSpec>,
        Vec<String>,
        usize,
    ),
    String,
> {
    let mut render_prefabs = Vec::new();
    let mut simulation_prefabs = Vec::new();
    let mut placement_ids = Vec::new();
    let mut metadata_only_count = 0usize;
    for placement in resolved
        .cell
        .placements
        .iter()
        .filter(|placement| placement.enabled)
    {
        placement_ids.push(placement.id.clone());
        if placement_is_spawn(placement) {
            continue;
        }
        if placement
            .apply_mode
            .trim()
            .eq_ignore_ascii_case("metadata_only")
        {
            // Root map metadata is the correct home for global domain configuration. A streamed
            // metadata-only placement cannot safely mutate already-running domain state here.
            metadata_only_count = metadata_only_count.saturating_add(1);
            continue;
        }
        let definition = resolve_definition(definition_cache, &placement.definition_ref)?;
        let drawable_ref = definition
            .refs
            .drawable_refs
            .first()
            .cloned()
            .ok_or_else(|| {
                format!(
                    "streamed placement '{}' definition_ref='{}' has no drawable_refs",
                    placement.id, placement.definition_ref
                )
            })?;
        let material_ref = definition
            .refs
            .material_refs
            .first()
            .cloned()
            .unwrap_or_default();
        let surface_binding = definition_surface_binding(&definition);
        let position = Vec3::new(
            placement.transform.position[0],
            placement.transform.position[1],
            placement.transform.position[2],
        );
        let rotation_ypr = Vec3::new(
            placement.transform.rotation_ypr[0],
            placement.transform.rotation_ypr[1],
            placement.transform.rotation_ypr[2],
        );
        let scale = Vec3::new(
            placement.transform.scale[0],
            placement.transform.scale[1],
            placement.transform.scale[2],
        );
        let dynamic_physics = placement
            .apply_mode
            .trim()
            .eq_ignore_ascii_case("dynamic_physics");
        let collision_only = placement
            .apply_mode
            .trim()
            .eq_ignore_ascii_case("collision_only")
            || placement
                .tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case("collision_only"));

        if !collision_only {
            let prefab = GameReadyPrefabSpec {
                id: placement.id.clone(),
                authored_map_ref: logical_map_ref.to_owned(),
                authored_placement_id: placement.id.clone(),
                authored_cell: Some(resolved.cell.coord),
                authored_discrete_placement: true,
                authored_primary: true,
                source: drawable_ref.clone(),
                proxy: if dynamic_physics {
                    DYNAMIC_WORLD_PROXY.to_owned()
                } else {
                    STATIC_WORLD_PROXY.to_owned()
                },
                material: material_ref,
                surface_id: surface_binding.id.clone(),
                surface_events: surface_binding.events.clone(),
                ground_placement_surface: surface_binding.ground_placement_surface,
                enabled: true,
                position,
                rotation_ypr,
                scale,
            };
            if dynamic_physics {
                simulation_prefabs.push(prefab);
            } else {
                render_prefabs.push(prefab);
            }
        }

        let collision_policy = definition.model_explanation.collision_policy.trim();
        let has_collision = !definition.refs.collision_refs.is_empty()
            || definition
                .semantic_tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case("collision"))
            || matches!(
                collision_policy.to_ascii_lowercase().as_str(),
                "static_mesh" | "triangle_mesh" | "mesh" | "box"
            );
        if has_collision && !dynamic_physics {
            let collision_source = definition
                .refs
                .collision_refs
                .first()
                .cloned()
                .unwrap_or(drawable_ref);
            simulation_prefabs.push(GameReadyPrefabSpec {
                id: if collision_only {
                    placement.id.clone()
                } else {
                    format!("{}#collision", placement.id)
                },
                authored_map_ref: logical_map_ref.to_owned(),
                authored_placement_id: placement.id.clone(),
                authored_cell: Some(resolved.cell.coord),
                authored_discrete_placement: true,
                authored_primary: false,
                source: collision_source,
                proxy: if collision_policy.eq_ignore_ascii_case("box") {
                    BOX_COLLISION_WORLD_PROXY.to_owned()
                } else {
                    COLLISION_WORLD_PROXY.to_owned()
                },
                material: String::new(),
                surface_id: surface_binding.id.clone(),
                surface_events: surface_binding.events.clone(),
                ground_placement_surface: surface_binding.ground_placement_surface,
                enabled: true,
                position,
                rotation_ypr,
                scale,
            });
        } else if collision_only {
            return Err(format!(
                "streamed collision_only placement '{}' definition_ref='{}' declares no collision",
                placement.id, placement.definition_ref
            ));
        }
    }
    Ok((
        render_prefabs,
        simulation_prefabs,
        placement_ids,
        metadata_only_count,
    ))
}

pub(super) fn prepare_cell(
    map_ref: &str,
    logical_map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
    definition_cache: &DefinitionCache,
) -> Result<PreparedMapCell, String> {
    let resolved = load_cell(map_ref, coord)?;
    let authored_placement_count = resolved.cell.placements.len();
    let (render_prefabs, simulation_prefabs, placement_ids, metadata_only_count) =
        cell_prefabs(logical_map_ref, &resolved, definition_cache)?;
    Ok(PreparedMapCell {
        render_prefabs,
        simulation_prefabs,
        placement_ids,
        authored_placement_count,
        metadata_only_count,
    })
}

#[cfg(test)]
mod project_surface_metadata_tests {
    use super::*;

    #[test]
    fn project_surface_metadata_is_explicit_and_generic() {
        let mut entry = ResolvedMapDefinitionEntry::default();
        entry.arbitrary_metadata.insert(
            "metadata".to_owned(),
            serde_json::json!({
                "newengine.physics.surface": {
                    "id": "project.deck.grating",
                    "events": {
                        "contact": "project.contact.boot_grating",
                        "landing": "project.contact.land_grating",
                        "project.custom_signal": "project.anything.custom"
                    },
                    "ground_placement_surface": true
                }
            }),
        );
        let binding = definition_surface_binding(&entry);
        assert_eq!(binding.id, "project.deck.grating");
        assert_eq!(
            binding.events.get("contact").map(String::as_str),
            Some("project.contact.boot_grating")
        );
        assert_eq!(
            binding
                .events
                .get("project.custom_signal")
                .map(String::as_str),
            Some("project.anything.custom")
        );
        assert!(binding.ground_placement_surface);
    }

    #[test]
    fn absent_surface_metadata_stays_neutral() {
        let binding = definition_surface_binding(&ResolvedMapDefinitionEntry::default());
        assert!(binding.id.is_empty());
        assert!(binding.events.is_empty());
        assert!(!binding.ground_placement_surface);
    }
}
