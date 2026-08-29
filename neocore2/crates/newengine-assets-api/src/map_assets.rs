#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::AssetDependencyRecord;

/// Canonical semantic schema for the root entry of a discrete `.ymap` ListFile.
pub const MAP_INDEX_SCHEMA_V1: &str = "newengine.map.index.v1";
/// Canonical semantic schema for one independently addressable map cell entry.
pub const MAP_CELL_SCHEMA_V1: &str = "newengine.map.cell.v1";
/// Conventional root selector inside every discrete `.ymap`.
pub const MAP_INDEX_ENTRY: &str = "map";
/// Default world-space size of one map cell.
pub const MAP_DEFAULT_CELL_SIZE: f32 = 64.0;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct MapCellCoordV1 {
    pub x: i32,
    pub z: i32,
}

impl MapCellCoordV1 {
    #[inline]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    #[inline]
    pub fn canonical_entry(self) -> String {
        format!("cell/{}/{}", self.x, self.z)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapTransformV1 {
    pub position: [f32; 3],
    pub rotation_ypr: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for MapTransformV1 {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation_ypr: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

impl MapTransformV1 {
    pub fn validate(&self) -> Result<(), String> {
        let finite = self
            .position
            .iter()
            .chain(self.rotation_ypr.iter())
            .chain(self.scale.iter())
            .all(|value| value.is_finite());
        if !finite {
            return Err("map transform contains a non-finite value".to_owned());
        }
        if self.scale.iter().any(|value| *value <= 0.0) {
            return Err("map transform scale must be positive on every axis".to_owned());
        }
        Ok(())
    }
}

/// One declarative placement inside a cell.
///
/// `definition_ref` is deliberately a `.ytyp@entry` reference. A map composes
/// authored definitions; it does not directly own model/material/texture semantics.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapPlacementV1 {
    pub id: String,
    pub definition_ref: String,
    pub transform: MapTransformV1,
    pub apply_mode: String,
    pub tags: Vec<String>,
    pub enabled: bool,
}

impl Default for MapPlacementV1 {
    fn default() -> Self {
        Self {
            id: String::new(),
            definition_ref: String::new(),
            transform: MapTransformV1::default(),
            apply_mode: "instantiate".to_owned(),
            tags: Vec::new(),
            enabled: true,
        }
    }
}

impl MapPlacementV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("map placement id is empty".to_owned());
        }
        let definitionless_marker = matches!(
            self.apply_mode.trim().to_ascii_lowercase().as_str(),
            "player_spawn" | "info_player_start"
        ) || self.tags.iter().any(|tag| {
            matches!(
                tag.trim().to_ascii_lowercase().as_str(),
                "player_spawn" | "info_player_start" | "spawn.player"
            )
        });
        if self.definition_ref.trim().is_empty() {
            if !definitionless_marker {
                require_definition_ref(&self.definition_ref)?;
            }
        } else {
            require_definition_ref(&self.definition_ref)?;
        }
        if self.apply_mode.trim().is_empty() {
            return Err(format!("map placement '{}' apply_mode is empty", self.id));
        }
        self.transform
            .validate()
            .map_err(|error| format!("map placement '{}': {error}", self.id))?;
        Ok(())
    }

    pub fn normalize(&mut self) {
        self.id = self.id.trim().to_owned();
        self.definition_ref = normalize_asset_ref(&self.definition_ref);
        self.apply_mode = self.apply_mode.trim().to_ascii_lowercase();
        normalize_string_set(&mut self.tags);
    }
}

/// Address of one independently replaceable cell entry inside the same `.ymap`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapCellRefV1 {
    pub coord: MapCellCoordV1,
    pub entry: String,
    pub required: bool,
}

impl Default for MapCellRefV1 {
    fn default() -> Self {
        Self {
            coord: MapCellCoordV1::default(),
            entry: String::new(),
            required: true,
        }
    }
}

impl MapCellRefV1 {
    #[inline]
    pub fn canonical(coord: MapCellCoordV1) -> Self {
        Self {
            coord,
            entry: coord.canonical_entry(),
            required: true,
        }
    }

    pub fn normalize(&mut self) {
        self.entry = self.entry.trim().trim_start_matches('@').to_owned();
        if self.entry.is_empty() {
            self.entry = self.coord.canonical_entry();
        }
    }
}

/// Optional composition of another map as a layer.
///
/// This is the map-level replacement/modding seam. A layer is another canonical
/// `.ymap@map` reference selected by registry/profile policy, not an embedded copy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapLayerRefV1 {
    pub id: String,
    pub map_ref: String,
    pub mode: String,
    pub priority: i32,
    pub enabled: bool,
}

impl Default for MapLayerRefV1 {
    fn default() -> Self {
        Self {
            id: String::new(),
            map_ref: String::new(),
            mode: "additive".to_owned(),
            priority: 0,
            enabled: true,
        }
    }
}

impl MapLayerRefV1 {
    pub fn normalize(&mut self) {
        self.id = self.id.trim().to_owned();
        self.map_ref = normalize_asset_ref(&self.map_ref);
        self.mode = self.mode.trim().to_ascii_lowercase();
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("map layer id is empty".to_owned());
        }
        require_map_ref(&self.map_ref)?;
        match self.mode.as_str() {
            "additive" | "override" => Ok(()),
            other => Err(format!(
                "map layer '{}' has unsupported mode '{other}'; expected additive|override",
                self.id
            )),
        }
    }
}

/// Root map entry. It owns topology only: cell index, map layers and lightweight metadata.
/// Heavy placement data lives in `MapCellV1` entries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapIndexV1 {
    pub schema: String,
    pub map_id: String,
    pub origin: [f32; 3],
    pub cell_size: f32,
    pub cells: Vec<MapCellRefV1>,
    pub layers: Vec<MapLayerRefV1>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for MapIndexV1 {
    fn default() -> Self {
        Self {
            schema: MAP_INDEX_SCHEMA_V1.to_owned(),
            map_id: String::new(),
            origin: [0.0; 3],
            cell_size: MAP_DEFAULT_CELL_SIZE,
            cells: Vec::new(),
            layers: Vec::new(),
            tags: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl MapIndexV1 {
    pub fn normalize(&mut self) {
        self.schema = MAP_INDEX_SCHEMA_V1.to_owned();
        self.map_id = self.map_id.trim().to_owned();
        for cell in &mut self.cells {
            cell.normalize();
        }
        self.cells.sort_by_key(|cell| (cell.coord.x, cell.coord.z));
        for layer in &mut self.layers {
            layer.normalize();
        }
        self.layers.sort_by(|left, right| {
            left.priority
                .cmp(&right.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        normalize_string_set(&mut self.tags);
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.schema != MAP_INDEX_SCHEMA_V1 {
            errors.push(format!(
                "map index schema '{}' is unsupported; expected '{}'",
                self.schema, MAP_INDEX_SCHEMA_V1
            ));
        }
        if self.map_id.trim().is_empty() {
            errors.push("map index map_id is empty".to_owned());
        }
        if !self.cell_size.is_finite() || self.cell_size <= 0.0 {
            errors.push("map index cell_size must be finite and > 0".to_owned());
        }
        if self.origin.iter().any(|value| !value.is_finite()) {
            errors.push("map index origin contains a non-finite value".to_owned());
        }

        let mut coords = BTreeSet::new();
        let mut entries = BTreeSet::new();
        for cell in &self.cells {
            if !coords.insert(cell.coord) {
                errors.push(format!(
                    "duplicate map cell coordinate {},{}",
                    cell.coord.x, cell.coord.z
                ));
            }
            if cell.entry.trim().is_empty() {
                errors.push(format!(
                    "map cell {},{} has an empty entry selector",
                    cell.coord.x, cell.coord.z
                ));
            } else if !entries.insert(cell.entry.as_str()) {
                errors.push(format!(
                    "duplicate map cell entry selector '{}'",
                    cell.entry
                ));
            }
        }

        let mut layer_ids = BTreeSet::new();
        for layer in &self.layers {
            if !layer_ids.insert(layer.id.as_str()) {
                errors.push(format!("duplicate map layer id '{}'", layer.id));
            }
            if let Err(error) = layer.validate() {
                errors.push(error);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[inline]
    pub fn cell(&self, coord: MapCellCoordV1) -> Option<&MapCellRefV1> {
        self.cells.iter().find(|cell| cell.coord == coord)
    }

    pub fn world_to_cell(&self, position: [f32; 3]) -> Option<MapCellCoordV1> {
        if !self.cell_size.is_finite() || self.cell_size <= 0.0 {
            return None;
        }
        let x = ((position[0] - self.origin[0]) / self.cell_size).floor();
        let z = ((position[2] - self.origin[2]) / self.cell_size).floor();
        if x < i32::MIN as f32 || x > i32::MAX as f32 || z < i32::MIN as f32 || z > i32::MAX as f32
        {
            return None;
        }
        Some(MapCellCoordV1::new(x as i32, z as i32))
    }
}

/// Independently addressable placement payload. A cell may be replaced without
/// changing the root map entry or neighboring cells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapCellV1 {
    pub schema: String,
    pub coord: MapCellCoordV1,
    pub placements: Vec<MapPlacementV1>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for MapCellV1 {
    fn default() -> Self {
        Self {
            schema: MAP_CELL_SCHEMA_V1.to_owned(),
            coord: MapCellCoordV1::default(),
            placements: Vec::new(),
            tags: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl MapCellV1 {
    pub fn normalize(&mut self) {
        self.schema = MAP_CELL_SCHEMA_V1.to_owned();
        for placement in &mut self.placements {
            placement.normalize();
        }
        self.placements
            .sort_by(|left, right| left.id.cmp(&right.id));
        normalize_string_set(&mut self.tags);
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.schema != MAP_CELL_SCHEMA_V1 {
            errors.push(format!(
                "map cell schema '{}' is unsupported; expected '{}'",
                self.schema, MAP_CELL_SCHEMA_V1
            ));
        }
        let mut ids = BTreeSet::new();
        for placement in &self.placements {
            if !ids.insert(placement.id.as_str()) {
                errors.push(format!("duplicate map placement id '{}'", placement.id));
            }
            if let Err(error) = placement.validate() {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MapRefRequestV1 {
    /// Canonical form: `maps/world.ymap@map`.
    pub map_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MapCellRequestV1 {
    /// Canonical form: `maps/world.ymap@map` or `maps/world.ymap`.
    pub map_ref: String,
    pub coord: MapCellCoordV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MapResolvedCellV1 {
    pub map_ref: String,
    pub cell_ref: String,
    pub index: MapIndexV1,
    pub cell: MapCellV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MapDependenciesV1 {
    pub map_ref: String,
    pub dependencies: Vec<AssetDependencyRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MapValidationV1 {
    pub ok: bool,
    pub map_ref: String,
    pub cell_count: u32,
    pub placement_count: u32,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[inline]
pub fn map_entry_ref(logical_path: &str, entry: &str) -> String {
    let normalized = normalize_asset_ref(logical_path);
    let logical_path = normalized.split('@').next().unwrap_or_default().to_owned();
    format!("{logical_path}@{}", entry.trim().trim_start_matches('@'))
}

pub fn require_map_ref(value: &str) -> Result<(), String> {
    require_extension_and_entry(value, "ymap", true)
        .map_err(|error| format!("invalid map ref '{value}': {error}"))
}

pub fn require_definition_ref(value: &str) -> Result<(), String> {
    require_extension_and_entry(value, "ytyp", true)
        .map_err(|error| format!("invalid definition ref '{value}': {error}"))
}

fn require_extension_and_entry(
    value: &str,
    extension: &str,
    entry_required: bool,
) -> Result<(), String> {
    let normalized = normalize_asset_ref(value);
    if normalized.is_empty() {
        return Err("reference is empty".to_owned());
    }
    let (path, entry) = match normalized.split_once('@') {
        Some((path, entry)) => (path, Some(entry)),
        None => (normalized.as_str(), None),
    };
    if !path
        .to_ascii_lowercase()
        .ends_with(&format!(".{extension}"))
    {
        return Err(format!("expected .{extension} path"));
    }
    if entry_required
        && entry
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .is_none()
    {
        return Err("addressable entry selector is required".to_owned());
    }
    Ok(())
}

fn normalize_asset_ref(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_slash = false;
    for character in value.trim().chars() {
        let character = if character == '\\' { '/' } else { character };
        if character == '/' {
            if previous_slash {
                continue;
            }
            previous_slash = true;
        } else {
            previous_slash = false;
        }
        output.push(character);
    }
    while output.starts_with("./") {
        output.drain(..2);
    }
    while output.starts_with('/') {
        output.remove(0);
    }
    output
}

fn normalize_string_set(values: &mut Vec<String>) {
    for value in values.iter_mut() {
        *value = value.trim().to_ascii_lowercase();
    }
    values.retain(|value| !value.is_empty());
    values.sort();
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(id: &str, definition_ref: &str) -> MapPlacementV1 {
        MapPlacementV1 {
            id: id.to_owned(),
            definition_ref: definition_ref.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn canonical_cell_entry_is_stable() {
        assert_eq!(MapCellCoordV1::new(-2, 7).canonical_entry(), "cell/-2/7");
    }

    #[test]
    fn index_rejects_duplicate_cells() {
        let mut index = MapIndexV1 {
            map_id: "world".to_owned(),
            cells: vec![
                MapCellRefV1::canonical(MapCellCoordV1::new(0, 0)),
                MapCellRefV1::canonical(MapCellCoordV1::new(0, 0)),
            ],
            ..Default::default()
        };
        index.normalize();
        let errors = index.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("duplicate map cell coordinate")));
    }

    #[test]
    fn cell_rejects_direct_model_refs() {
        let cell = MapCellV1 {
            placements: vec![placement("tower", "models/tower.ydd@tower")],
            ..Default::default()
        };
        let errors = cell.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("expected .ytyp path")));
    }

    #[test]
    fn cell_accepts_definitionless_player_spawn_marker() {
        let cell = MapCellV1 {
            placements: vec![MapPlacementV1 {
                id: "player_start".to_owned(),
                definition_ref: String::new(),
                apply_mode: "player_spawn".to_owned(),
                tags: vec!["player_spawn".to_owned()],
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(cell.validate().is_ok());
    }

    #[test]
    fn cell_rejects_definitionless_instantiated_placement() {
        let cell = MapCellV1 {
            placements: vec![placement("orphan", "")],
            ..Default::default()
        };
        let errors = cell.validate().unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("reference is empty")));
    }

    #[test]
    fn cell_accepts_definition_entries() {
        let cell = MapCellV1 {
            placements: vec![placement("tower", "definitions/world.ytyp@tower")],
            ..Default::default()
        };
        assert!(cell.validate().is_ok());
    }

    #[test]
    fn world_position_maps_to_discrete_cell() {
        let index = MapIndexV1 {
            map_id: "world".to_owned(),
            origin: [-64.0, 0.0, -64.0],
            cell_size: 64.0,
            ..Default::default()
        };
        assert_eq!(
            index.world_to_cell([0.0, 100.0, 0.0]),
            Some(MapCellCoordV1::new(1, 1))
        );
    }

    #[test]
    fn normalization_makes_cell_payload_deterministic() {
        let mut cell = MapCellV1 {
            placements: vec![
                placement("b", "definitions\\world.ytyp@b"),
                placement("a", "definitions/world.ytyp@a"),
            ],
            tags: vec![
                " World ".to_owned(),
                "world".to_owned(),
                "STREAMING".to_owned(),
            ],
            ..Default::default()
        };
        cell.normalize();
        assert_eq!(cell.placements[0].id, "a");
        assert_eq!(
            cell.placements[1].definition_ref,
            "definitions/world.ytyp@b"
        );
        assert_eq!(cell.tags, vec!["streaming", "world"]);
    }
}
