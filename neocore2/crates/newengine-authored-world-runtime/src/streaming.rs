use std::collections::BTreeMap;
use std::sync::Arc;

use newengine_definitions_runtime::DefinitionEntryV1;
use newengine_math::Vec3;
use parking_lot::Mutex;

pub const WORLD_STATIC_PROXY: &str = "world_static_ydd";
pub const WORLD_DYNAMIC_PROXY: &str = "world_dynamic_ydd";
pub const WORLD_COLLISION_PROXY: &str = "world_collision_ydd";
pub const WORLD_COLLISION_BOX_PROXY: &str = "world_collision_box";

#[derive(Clone, Debug)]
pub struct AuthoredMapStreamingSpec {
    pub map_ref: String,
    pub index: newengine_assets_api::MapIndexV1,
    pub initial_render_cells: Vec<newengine_assets_api::MapCellCoordV1>,
    pub initial_simulation_cells: Vec<newengine_assets_api::MapCellCoordV1>,
    pub initial_placement_ids: BTreeMap<newengine_assets_api::MapCellCoordV1, Vec<String>>,
    pub render_radius: i32,
    pub simulation_radius: i32,
    pub render_unload_radius: i32,
    pub simulation_unload_radius: i32,
    pub max_cells_per_tick: usize,
}

#[derive(Clone, Debug)]
pub struct AuthoredWorldPlacementSpec {
    pub id: String,
    pub authored_map_ref: String,
    pub authored_placement_id: String,
    pub authored_cell: Option<newengine_assets_api::MapCellCoordV1>,
    pub authored_discrete_placement: bool,
    pub authored_primary: bool,
    pub source: String,
    pub proxy: String,
    pub material: String,
    pub surface_id: String,
    pub surface_events: BTreeMap<String, String>,
    pub ballistic_material: Option<newengine_engine_runtime::gameplay::BallisticMaterialResponse>,
    pub ground_placement_surface: bool,
    pub enabled: bool,
    pub position: Vec3,
    pub rotation_ypr: Vec3,
    pub scale: Vec3,
}

#[derive(Clone, Debug)]
pub struct PreparedAuthoredMapCell {
    pub render_placements: Vec<AuthoredWorldPlacementSpec>,
    pub simulation_placements: Vec<AuthoredWorldPlacementSpec>,
    pub placement_ids: Vec<String>,
    pub authored_placement_count: usize,
    pub metadata_only_count: usize,
}

#[derive(Clone, Default)]
pub struct AuthoredMapDefinitionCache {
    entries: Arc<Mutex<BTreeMap<String, DefinitionEntryV1>>>,
}

impl AuthoredMapDefinitionCache {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Default)]
pub struct AuthoredDefinitionSurfaceBinding {
    pub id: String,
    pub events: BTreeMap<String, String>,
    pub ballistic_material: Option<newengine_engine_runtime::gameplay::BallisticMaterialResponse>,
    pub ground_placement_surface: bool,
}

fn resolve_definition(
    cache: &AuthoredMapDefinitionCache,
    definition_ref: &str,
) -> Result<DefinitionEntryV1, String> {
    if let Some(existing) = cache.entries.lock().get(definition_ref).cloned() {
        return Ok(existing);
    }
    let parsed = crate::load_authored_definition_entry(definition_ref)?;
    let mut locked = cache.entries.lock();
    Ok(locked
        .entry(definition_ref.to_owned())
        .or_insert_with(|| parsed.clone())
        .clone())
}

pub fn project_authored_definition_surface(
    definition: &DefinitionEntryV1,
) -> AuthoredDefinitionSurfaceBinding {
    fn parse(value: &serde_json::Value) -> Option<AuthoredDefinitionSurfaceBinding> {
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
        let ballistic_material = object
            .get("ballistics")
            .and_then(serde_json::Value::as_object)
            .map(|ballistics| {
                let f32_or = |key: &str, default: f32| {
                    ballistics
                        .get(key)
                        .and_then(|value| {
                            value
                                .as_f64()
                                .map(|value| value as f32)
                                .or_else(|| value.as_str()?.parse::<f32>().ok())
                        })
                        .unwrap_or(default)
                };
                let bool_or = |key: &str, default: bool| {
                    ballistics
                        .get(key)
                        .and_then(|value| {
                            value.as_bool().or_else(|| {
                                match value.as_str()?.trim().to_ascii_lowercase().as_str() {
                                    "1" | "true" | "yes" | "on" => Some(true),
                                    "0" | "false" | "no" | "off" => Some(false),
                                    _ => None,
                                }
                            })
                        })
                        .unwrap_or(default)
                };
                newengine_engine_runtime::gameplay::BallisticMaterialResponse {
                    penetration_resistance_j_per_m: f32_or(
                        "penetration_resistance_j_per_m",
                        f32::INFINITY,
                    ),
                    entry_energy_cost_j: f32_or("entry_energy_cost_j", f32::INFINITY),
                    damage_transfer_multiplier: f32_or("damage_transfer_multiplier", 1.0),
                    impulse_transfer_multiplier: f32_or("impulse_transfer_multiplier", 1.0),
                    ricochet_allowed: bool_or("ricochet_allowed", false),
                    ricochet_max_incidence_dot: f32_or("ricochet_max_incidence_dot", 0.0),
                    ricochet_energy_retention: f32_or("ricochet_energy_retention", 0.0),
                }
                .sanitized()
            });
        let ground_placement_surface = object
            .get("ground_placement_surface")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        Some(AuthoredDefinitionSurfaceBinding {
            id,
            events,
            ballistic_material,
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
        if let Some(binding) = root.get("engine.physics.surface").and_then(parse) {
            return binding;
        }
    }
    AuthoredDefinitionSurfaceBinding::default()
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

fn project_cell_placements(
    logical_map_ref: &str,
    resolved: &newengine_assets_api::MapResolvedCellV2,
    definition_cache: &AuthoredMapDefinitionCache,
) -> Result<
    (
        Vec<AuthoredWorldPlacementSpec>,
        Vec<AuthoredWorldPlacementSpec>,
        Vec<String>,
        usize,
    ),
    String,
> {
    let mut render_placements = Vec::new();
    let mut simulation_placements = Vec::new();
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
        let surface_binding = project_authored_definition_surface(&definition);
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
            let authored = AuthoredWorldPlacementSpec {
                id: placement.id.clone(),
                authored_map_ref: logical_map_ref.to_owned(),
                authored_placement_id: placement.id.clone(),
                authored_cell: Some(resolved.cell.coord),
                authored_discrete_placement: true,
                authored_primary: true,
                source: drawable_ref.clone(),
                proxy: if dynamic_physics {
                    WORLD_DYNAMIC_PROXY.to_owned()
                } else {
                    WORLD_STATIC_PROXY.to_owned()
                },
                material: material_ref,
                surface_id: surface_binding.id.clone(),
                surface_events: surface_binding.events.clone(),
                ballistic_material: surface_binding.ballistic_material,
                ground_placement_surface: surface_binding.ground_placement_surface,
                enabled: true,
                position,
                rotation_ypr,
                scale,
            };
            if dynamic_physics {
                simulation_placements.push(authored);
            } else {
                render_placements.push(authored);
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
            simulation_placements.push(AuthoredWorldPlacementSpec {
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
                    WORLD_COLLISION_BOX_PROXY.to_owned()
                } else {
                    WORLD_COLLISION_PROXY.to_owned()
                },
                material: String::new(),
                surface_id: surface_binding.id.clone(),
                surface_events: surface_binding.events.clone(),
                ballistic_material: surface_binding.ballistic_material,
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
        render_placements,
        simulation_placements,
        placement_ids,
        metadata_only_count,
    ))
}

pub fn prepare_authored_map_cell(
    map_ref: &str,
    logical_map_ref: &str,
    coord: newengine_assets_api::MapCellCoordV1,
    definition_cache: &AuthoredMapDefinitionCache,
) -> Result<PreparedAuthoredMapCell, String> {
    let resolved = crate::load_authored_map_cell(map_ref, coord)?;
    let authored_placement_count = resolved.cell.placements.len();
    let (render_placements, simulation_placements, placement_ids, metadata_only_count) =
        project_cell_placements(logical_map_ref, &resolved, definition_cache)?;
    Ok(PreparedAuthoredMapCell {
        render_placements,
        simulation_placements,
        placement_ids,
        authored_placement_count,
        metadata_only_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_surface_metadata_is_explicit_and_generic() {
        let mut entry = DefinitionEntryV1::default();
        entry.arbitrary_metadata.insert(
            "metadata".to_owned(),
            serde_json::json!({
                "engine.physics.surface": {
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
        let binding = project_authored_definition_surface(&entry);
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
        let binding = project_authored_definition_surface(&DefinitionEntryV1::default());
        assert!(binding.id.is_empty());
        assert!(binding.events.is_empty());
        assert!(!binding.ground_placement_surface);
    }
}
