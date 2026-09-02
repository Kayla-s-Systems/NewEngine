#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageImportRequestV1 {
    pub schema: String,
    pub settings: FoliageSettings,
    pub target_platform: String,
}

impl Default for FoliageImportRequestV1 {
    fn default() -> Self {
        Self {
            schema: FOLIAGE_IMPORT_REQUEST_SCHEMA.to_owned(),
            settings: FoliageSettings::default(),
            target_platform: "any".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageImportResponseV1 {
    pub schema: String,
    pub accepted: bool,
    pub canonical_source_ref: String,
    pub runtime_asset_ref: String,
    pub importer_id: String,
    pub asset_id: String,
    pub queue_status: String,
    pub warnings: Vec<String>,
}

impl Default for FoliageImportResponseV1 {
    fn default() -> Self {
        Self {
            schema: FOLIAGE_IMPORT_RESPONSE_SCHEMA.to_owned(),
            accepted: false,
            canonical_source_ref: String::new(),
            runtime_asset_ref: String::new(),
            importer_id: FOLIAGE_SRT_IMPORTER_ID.to_owned(),
            asset_id: String::new(),
            queue_status: String::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageLodAssetV1 {
    pub lod_index: u16,
    pub min_distance: f32,
    pub max_distance: f32,
    pub drawable_ref: String,
    pub impostor: bool,
}

impl Default for FoliageLodAssetV1 {
    fn default() -> Self {
        Self {
            lod_index: 0,
            min_distance: 0.0,
            max_distance: 32_768.0,
            drawable_ref: String::new(),
            impostor: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageMaterialBindingV1 {
    pub variant: String,
    pub material: MaterialId,
}

impl Default for FoliageMaterialBindingV1 {
    fn default() -> Self {
        Self {
            variant: "default".to_owned(),
            material: MaterialId::invalid(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageRuntimeAssetV1 {
    pub schema: String,
    pub source_ref: String,
    pub source_content_hash: String,
    pub importer_id: String,
    pub lods: Vec<FoliageLodAssetV1>,
    pub materials: Vec<FoliageMaterialBindingV1>,
    pub billboard_atlas_ref: Option<String>,
}

impl Default for FoliageRuntimeAssetV1 {
    fn default() -> Self {
        Self {
            schema: FOLIAGE_RUNTIME_SCHEMA.to_owned(),
            source_ref: String::new(),
            source_content_hash: String::new(),
            importer_id: FOLIAGE_SRT_IMPORTER_ID.to_owned(),
            lods: Vec::new(),
            materials: Vec::new(),
            billboard_atlas_ref: None,
        }
    }
}

impl FoliageRuntimeAssetV1 {
    pub fn validate(&self) -> Result<(), String> {
        if speedtree_importer_id(&self.source_ref).is_none() {
            return Err(
                "foliage runtime asset must retain its canonical .srt or .spm source_ref"
                    .to_owned(),
            );
        }
        if self.lods.is_empty() {
            return Err("foliage runtime asset has no LOD drawables".to_owned());
        }
        if self
            .lods
            .iter()
            .any(|lod| lod.drawable_ref.trim().is_empty())
        {
            return Err("foliage runtime asset contains an empty LOD drawable_ref".to_owned());
        }
        if self
            .lods
            .iter()
            .enumerate()
            .any(|(expected, lod)| lod.lod_index as usize != expected)
        {
            return Err(
                "foliage runtime asset LOD indices must be contiguous from zero".to_owned(),
            );
        }
        if self.lods.iter().any(|lod| {
            !lod.min_distance.is_finite()
                || !lod.max_distance.is_finite()
                || lod.min_distance < 0.0
                || lod.max_distance < lod.min_distance
        }) {
            return Err("foliage runtime asset contains an invalid LOD distance range".to_owned());
        }
        if self.materials.is_empty()
            || self
                .materials
                .iter()
                .any(|binding| !binding.material.is_valid())
        {
            return Err(
                "foliage runtime asset requires registry-backed material handles".to_owned(),
            );
        }
        Ok(())
    }

    pub fn material_for_variant(&self, variant: &str) -> MaterialId {
        let variant = variant.trim();
        self.materials
            .iter()
            .find(|binding| binding.variant == variant)
            .or_else(|| {
                self.materials
                    .iter()
                    .find(|binding| binding.variant == "default")
            })
            .map(|binding| binding.material)
            .unwrap_or_else(MaterialId::invalid)
    }
}
