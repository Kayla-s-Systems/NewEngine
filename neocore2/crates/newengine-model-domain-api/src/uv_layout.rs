use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Authored UV layout / unwrap metadata carried by `.ytyd` dictionaries.
///
/// This is model-domain metadata. Render backends consume already-resolved mesh
/// vertex streams and material packets; they must not parse `.ytyp` / `.ytyd`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UvLayoutDictionary {
    pub schema: String,
    pub source: String,
    pub entries: Vec<UvLayoutEntry>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for UvLayoutDictionary {
    fn default() -> Self {
        Self {
            schema: crate::UV_LAYOUT_DICTIONARY_SCHEMA.to_owned(),
            source: String::new(),
            entries: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UvLayoutEntry {
    pub name: String,
    pub source_mesh_ref: String,
    pub target_drawable_ref: String,
    pub coordinate_space: String,
    pub channels: Vec<UvChannelLayout>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for UvLayoutEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            source_mesh_ref: String::new(),
            target_drawable_ref: String::new(),
            coordinate_space: "uv01".to_owned(),
            channels: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UvChannelLayout {
    pub channel: u8,
    pub semantic: String,
    pub set_name: String,
    pub projection: UvProjection,
    pub transform: UvTransform,
    pub islands: Vec<UvIsland>,
    pub material_slots: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for UvChannelLayout {
    fn default() -> Self {
        Self {
            channel: 0,
            semantic: "base_color".to_owned(),
            set_name: "uv0".to_owned(),
            projection: UvProjection::Authored,
            transform: UvTransform::default(),
            islands: Vec::new(),
            material_slots: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UvProjection {
    #[default]
    Authored,
    Planar,
    BoxProjected,
    Cylindrical,
    Spherical,
    TriplanarHint,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UvTransform {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation_degrees: f32,
}

impl Default for UvTransform {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation_degrees: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UvIsland {
    pub id: String,
    pub material_slot: String,
    pub rect: UvRect,
    pub index_range: Option<UvIndexRange>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UvRect {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
}

impl Default for UvRect {
    fn default() -> Self {
        Self {
            u_min: 0.0,
            v_min: 0.0,
            u_max: 1.0,
            v_max: 1.0,
        }
    }
}

impl UvRect {
    #[inline]
    pub fn is_finite_and_ordered(&self) -> bool {
        self.u_min.is_finite()
            && self.v_min.is_finite()
            && self.u_max.is_finite()
            && self.v_max.is_finite()
            && self.u_min <= self.u_max
            && self.v_min <= self.v_max
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UvIndexRange {
    pub start: u32,
    pub count: u32,
}

pub fn validate_uv_layout_dictionary(dictionary: &UvLayoutDictionary) -> Result<(), String> {
    if dictionary.schema.trim() != crate::UV_LAYOUT_DICTIONARY_SCHEMA {
        return Err(format!(
            "unsupported .ytyd schema expected='{}' actual='{}'",
            crate::UV_LAYOUT_DICTIONARY_SCHEMA,
            dictionary.schema
        ));
    }
    if dictionary.entries.is_empty() {
        return Err(".ytyd dictionary must contain at least one UV layout entry".to_owned());
    }
    for entry in &dictionary.entries {
        validate_uv_layout_entry(entry)?;
    }
    Ok(())
}

pub fn validate_uv_layout_entry(entry: &UvLayoutEntry) -> Result<(), String> {
    if entry.name.trim().is_empty() {
        return Err(".ytyd UV layout entry requires non-empty name".to_owned());
    }
    if entry.channels.is_empty() {
        return Err(format!(
            ".ytyd UV layout entry '{}' requires at least one channel",
            entry.name
        ));
    }
    for channel in &entry.channels {
        validate_uv_channel_layout(&entry.name, channel)?;
    }
    Ok(())
}

pub fn validate_uv_channel_layout(
    entry_name: &str,
    channel: &UvChannelLayout,
) -> Result<(), String> {
    if channel.set_name.trim().is_empty() {
        return Err(format!(
            ".ytyd UV layout entry '{}' channel {} requires set_name",
            entry_name, channel.channel
        ));
    }
    for island in &channel.islands {
        if !island.rect.is_finite_and_ordered() {
            return Err(format!(
                ".ytyd UV layout entry '{}' channel {} island '{}' has invalid rect",
                entry_name, channel.channel, island.id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ytyd_accepts_authored_string_refs_without_extension_validation() {
        let dictionary = UvLayoutDictionary {
            source: "world/industrial_yards.ytyd".to_owned(),
            entries: vec![UvLayoutEntry {
                name: "skydome_unwrap".to_owned(),
                source_mesh_ref: "any authored mesh string".to_owned(),
                target_drawable_ref: "models/skydome.ydd@sky".to_owned(),
                channels: vec![UvChannelLayout {
                    channel: 0,
                    semantic: "sky_gradient".to_owned(),
                    set_name: "uv0".to_owned(),
                    islands: vec![UvIsland {
                        id: "upper_hemisphere".to_owned(),
                        material_slot: "sky".to_owned(),
                        rect: UvRect {
                            u_min: 0.0,
                            v_min: 0.0,
                            u_max: 1.0,
                            v_max: 0.5,
                        },
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        validate_uv_layout_dictionary(&dictionary).unwrap();
    }

    #[test]
    fn invalid_uv_rect_is_rejected() {
        let err = validate_uv_channel_layout(
            "bad",
            &UvChannelLayout {
                islands: vec![UvIsland {
                    id: "broken".to_owned(),
                    rect: UvRect {
                        u_min: 1.0,
                        v_min: 0.0,
                        u_max: 0.0,
                        v_max: 1.0,
                    },
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("invalid rect"));
    }
}
