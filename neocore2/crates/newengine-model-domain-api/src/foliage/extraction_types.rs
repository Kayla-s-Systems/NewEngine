#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoliageExtractionPathV1 {
    #[default]
    CpuFallback,
    GpuIndirect,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageExtractionCapabilitiesV1 {
    pub gpu_culling: bool,
    pub indirect_draw: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageExtractionViewV1 {
    pub camera_position: [f32; 3],
    pub shadow_pass: bool,
}

impl Default for FoliageExtractionViewV1 {
    fn default() -> Self {
        Self {
            camera_position: [0.0; 3],
            shadow_pass: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageInstanceInputV1 {
    pub stable_id: u64,
    pub transform_cols: [[f32; 4]; 4],
    pub bounds_center: [f32; 3],
    pub bounds_radius: f32,
    pub material_variant: Option<String>,
}

impl Default for FoliageInstanceInputV1 {
    fn default() -> Self {
        Self {
            stable_id: 0,
            transform_cols: identity_cols(),
            bounds_center: [0.0; 3],
            bounds_radius: 0.001,
            material_variant: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageExtractionRequestV1 {
    pub settings: FoliageSettings,
    pub runtime_asset: FoliageRuntimeAssetV1,
    pub instances: Vec<FoliageInstanceInputV1>,
    pub view: FoliageExtractionViewV1,
    pub capabilities: FoliageExtractionCapabilitiesV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoliageInstanceCommandV1 {
    pub stable_id: u64,
    pub transform_cols: [[f32; 4]; 4],
    pub lod_index: u16,
    pub lod_fade: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoliageGpuCandidateV1 {
    pub stable_id: u64,
    pub transform_cols: [[f32; 4]; 4],
    pub bounds_center: [f32; 3],
    pub bounds_radius: f32,
    pub material: MaterialId,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageGpuWorkV1 {
    pub settings: FoliageSettings,
    pub lods: Vec<FoliageLodAssetV1>,
    pub view: FoliageExtractionViewV1,
    pub candidates: Vec<FoliageGpuCandidateV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FoliageDrawBatchV1 {
    pub drawable_ref: String,
    pub material: MaterialId,
    pub instances: Vec<FoliageInstanceCommandV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageExtractionPlanV1 {
    pub schema: String,
    pub path: FoliageExtractionPathV1,
    pub wind: FoliageWindSettings,
    pub batches: Vec<FoliageDrawBatchV1>,
    pub gpu_work: Option<FoliageGpuWorkV1>,
    pub input_instances: u32,
    pub density_rejected: u32,
    pub distance_culled: u32,
    pub warnings: Vec<String>,
}

impl Default for FoliageExtractionPlanV1 {
    fn default() -> Self {
        Self {
            schema: FOLIAGE_EXTRACTION_PLAN_SCHEMA.to_owned(),
            path: FoliageExtractionPathV1::CpuFallback,
            wind: FoliageWindSettings::default(),
            batches: Vec::new(),
            gpu_work: None,
            input_instances: 0,
            density_rejected: 0,
            distance_culled: 0,
            warnings: Vec::new(),
        }
    }
}
