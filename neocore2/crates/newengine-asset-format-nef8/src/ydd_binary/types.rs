//! Public binary YDD document model and selection helpers.

pub const YDD_BINARY_SCHEMA_VERSION: u32 = 2;
pub const YDD_BINARY_ENCODING: &str = "newengine.ydd.binary_mesh.v2";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YddBinaryVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv0: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct YddBinaryMesh {
    pub name: String,
    pub material_ref: Option<String>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub vertices: Vec<YddBinaryVertex>,
    pub indices: Vec<u32>,
}

impl YddBinaryMesh {
    #[inline]
    pub fn material_slot(&self) -> String {
        if let Some(reference) = self.material_ref.as_deref() {
            if let Some((_, selector)) = reference.rsplit_once('@') {
                let selector = selector.trim();
                if !selector.is_empty() {
                    return selector.to_owned();
                }
            }
        }
        let name = self.name.trim();
        if name.is_empty() {
            "material".to_owned()
        } else {
            name.to_owned()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct YddBinaryEntry {
    pub name: String,
    pub source_path: String,
    pub properties_ref: Option<String>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub meshes: Vec<YddBinaryMesh>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct YddBinaryDocument {
    pub entries: Vec<YddBinaryEntry>,
}

impl YddBinaryDocument {
    pub fn select_entry(
        &self,
        selector: Option<&str>,
        allow_single_entry_fallback: bool,
    ) -> Result<&YddBinaryEntry, String> {
        match selector.map(str::trim).filter(|value| !value.is_empty()) {
            Some(selector) => self
                .entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(selector))
                .or_else(|| {
                    (allow_single_entry_fallback && self.entries.len() == 1)
                        .then(|| &self.entries[0])
                })
                .ok_or_else(|| format!("binary YDD selector '{selector}' was not found")),
            None => self
                .entries
                .first()
                .ok_or_else(|| "binary YDD contains no entries".to_owned()),
        }
    }

    pub fn select_mesh(
        &self,
        selector: Option<&str>,
        allow_single_entry_fallback: bool,
    ) -> Result<(&YddBinaryEntry, &YddBinaryMesh), String> {
        if let Some(selector) = selector.map(str::trim).filter(|value| !value.is_empty()) {
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(selector))
            {
                return entry
                    .meshes
                    .first()
                    .map(|mesh| (entry, mesh))
                    .ok_or_else(|| format!("binary YDD entry '{selector}' contains no meshes"));
            }
            for entry in &self.entries {
                if let Some(mesh) = entry
                    .meshes
                    .iter()
                    .find(|mesh| mesh.name.eq_ignore_ascii_case(selector))
                {
                    return Ok((entry, mesh));
                }
            }
        }
        let entry = self.select_entry(selector, allow_single_entry_fallback)?;
        entry
            .meshes
            .first()
            .map(|mesh| (entry, mesh))
            .ok_or_else(|| format!("binary YDD entry '{}' contains no meshes", entry.name))
    }
}
