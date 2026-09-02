use std::collections::BTreeMap;

use newengine_materials::MaterialId;
use serde::{Deserialize, Serialize};

pub const SPEEDTREE_SRT_EXTENSION: &str = "srt";
pub const SPEEDTREE_SPM_EXTENSION: &str = "spm";
pub const FOLIAGE_RUNTIME_EXTENSION: &str = "nefoliage";
pub const FOLIAGE_RUNTIME_SCHEMA: &str = "newengine.foliage.runtime.v1";
pub const FOLIAGE_IMPORT_REQUEST_SCHEMA: &str = "newengine.foliage.import_request.v1";
pub const FOLIAGE_IMPORT_RESPONSE_SCHEMA: &str = "newengine.foliage.import_response.v1";
pub const FOLIAGE_EXTRACTION_PLAN_SCHEMA: &str = "newengine.foliage.extraction_plan.v1";
pub const FOLIAGE_SRT_IMPORTER_ID: &str = "northstar.importer.speedtree_srt.v1";
pub const FOLIAGE_SPM_IMPORTER_ID: &str = "northstar.importer.speedtree_spm.v1";
pub const FOLIAGE_GPU_CULLING_CAPABILITY_ID: &str = "render.foliage.gpu_culling";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageScaleSettings {
    pub min: f32,
    pub max: f32,
}

impl Default for FoliageScaleSettings {
    fn default() -> Self {
        Self { min: 1.0, max: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageWindSettings {
    pub enabled: bool,
    pub strength: f32,
    pub gust_frequency: f32,
    pub direction: [f32; 3],
}

impl Default for FoliageWindSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            strength: 0.42,
            gust_frequency: 0.17,
            direction: [0.78, 0.0, 0.62],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageLodSettings {
    /// Mesh transition distances in metres, ordered from near to far.
    pub mesh_distances: Vec<f32>,
    pub impostor_distance: f32,
    pub crossfade_width: f32,
}

impl Default for FoliageLodSettings {
    fn default() -> Self {
        Self {
            mesh_distances: vec![28.0, 64.0, 120.0],
            impostor_distance: 120.0,
            crossfade_width: 6.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageCullSettings {
    pub max_distance: f32,
    pub shadow_max_distance: f32,
}

impl Default for FoliageCullSettings {
    fn default() -> Self {
        Self {
            max_distance: 300.0,
            shadow_max_distance: 180.0,
        }
    }
}

/// Project/deployment-owned foliage policy.
///
/// The engine treats SpeedTree SRT/SPM data as opaque authoring sources. AssetManager
/// owns bytes and import scheduling, the model gateway owns the compiled foliage
/// packet, material handles come from MaterialRegistry, and render providers own
/// shader/GPU state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageSettings {
    pub canonical_path: String,
    pub scale: FoliageScaleSettings,
    pub wind: FoliageWindSettings,
    pub lod: FoliageLodSettings,
    pub cull: FoliageCullSettings,
    pub density: f32,
    pub material_variant: String,
    pub seed: u64,
    pub prefer_gpu_culling: bool,
}

impl Default for FoliageSettings {
    fn default() -> Self {
        Self {
            canonical_path: String::new(),
            scale: FoliageScaleSettings::default(),
            wind: FoliageWindSettings::default(),
            lod: FoliageLodSettings::default(),
            cull: FoliageCullSettings::default(),
            density: 1.0,
            material_variant: "default".to_owned(),
            seed: 0,
            prefer_gpu_culling: true,
        }
    }
}

impl FoliageSettings {
    pub fn sanitized(self) -> Result<Self, String> {
        let settings = self.sanitized_policy()?;
        if settings.canonical_path.is_empty() {
            return Err("foliage canonical_path is empty".to_owned());
        }
        Ok(settings)
    }

    /// Sanitize project/deployment policy before a canonical source is assigned.
    pub fn sanitized_policy(mut self) -> Result<Self, String> {
        self.canonical_path = normalize_logical_source(&self.canonical_path);
        if !self.canonical_path.is_empty() && speedtree_importer_id(&self.canonical_path).is_none()
        {
            return Err(format!(
                "foliage canonical_path '{}' must use .{} or .{} SpeedTree source",
                self.canonical_path, SPEEDTREE_SRT_EXTENSION, SPEEDTREE_SPM_EXTENSION
            ));
        }

        self.scale.min = finite_or(self.scale.min, 1.0).clamp(0.01, 64.0);
        self.scale.max = finite_or(self.scale.max, self.scale.min).clamp(self.scale.min, 64.0);

        self.wind.strength = finite_or(self.wind.strength, 0.42).clamp(0.0, 8.0);
        self.wind.gust_frequency = finite_or(self.wind.gust_frequency, 0.17).clamp(0.001, 32.0);
        self.wind.direction = normalized_direction(self.wind.direction);

        self.lod.mesh_distances = sanitize_lod_distances(self.lod.mesh_distances);
        let last_mesh_distance = self.lod.mesh_distances.last().copied().unwrap_or(120.0);
        self.lod.impostor_distance = finite_or(self.lod.impostor_distance, last_mesh_distance)
            .clamp(last_mesh_distance, 16_384.0);
        self.lod.crossfade_width = finite_or(self.lod.crossfade_width, 6.0).clamp(0.0, 128.0);

        self.cull.max_distance =
            finite_or(self.cull.max_distance, 300.0).clamp(self.lod.impostor_distance, 32_768.0);
        self.cull.shadow_max_distance =
            finite_or(self.cull.shadow_max_distance, 180.0).clamp(1.0, self.cull.max_distance);

        self.density = finite_or(self.density, 1.0).clamp(0.0, 1.0);
        self.material_variant = self.material_variant.trim().to_owned();
        if self.material_variant.is_empty() {
            self.material_variant = "default".to_owned();
        }
        Ok(self)
    }

    pub fn runtime_asset_ref(&self) -> Result<String, String> {
        let settings = self.clone().sanitized()?;
        let dot = settings
            .canonical_path
            .rfind('.')
            .expect("validated SpeedTree path has an extension");
        Ok(format!(
            "{}.{}",
            &settings.canonical_path[..dot],
            FOLIAGE_RUNTIME_EXTENSION
        ))
    }

    pub fn importer_id(&self) -> Result<&'static str, String> {
        let settings = self.clone().sanitized()?;
        speedtree_importer_id(&settings.canonical_path).ok_or_else(|| {
            format!(
                "foliage canonical_path '{}' has no registered SpeedTree importer",
                settings.canonical_path
            )
        })
    }

    #[inline]
    pub fn max_distance(&self, shadow_pass: bool) -> f32 {
        if shadow_pass {
            self.cull.shadow_max_distance
        } else {
            self.cull.max_distance
        }
    }

    pub fn selected_lod(&self, distance: f32, lod_count: u16) -> u16 {
        if lod_count <= 1 {
            return 0;
        }
        let distance = distance.max(0.0);
        let mut selected = 0usize;
        for threshold in &self.lod.mesh_distances {
            if distance < *threshold {
                break;
            }
            selected = selected.saturating_add(1);
        }
        selected.min(lod_count.saturating_sub(1) as usize) as u16
    }
}

/// Component attached to foliage render entities.
///
/// It is deliberately data-only. Render extraction may read it but must never
/// mutate the scene or start worker threads.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageInstanceRuntime {
    pub lod_index: u16,
    pub lod_count: u16,
    pub lod_min_distance: f32,
    pub lod_max_distance: f32,
    pub cull_max_distance: f32,
    pub shadow_cull_max_distance: f32,
    pub wind_enabled: bool,
    pub wind_strength: f32,
    pub wind_direction: [f32; 3],
}

impl Default for FoliageInstanceRuntime {
    fn default() -> Self {
        Self {
            lod_index: 0,
            lod_count: 1,
            lod_min_distance: 0.0,
            lod_max_distance: 32_768.0,
            cull_max_distance: 32_768.0,
            shadow_cull_max_distance: 32_768.0,
            wind_enabled: false,
            wind_strength: 0.0,
            wind_direction: [1.0, 0.0, 0.0],
        }
    }
}

impl FoliageInstanceRuntime {
    pub fn new(settings: &FoliageSettings, lod_index: u16, lod_count: u16) -> Self {
        let lod_count = lod_count.max(1);
        let lod_index = lod_index.min(lod_count - 1);
        let lod_min_distance = if lod_index == 0 {
            0.0
        } else {
            settings
                .lod
                .mesh_distances
                .get(lod_index.saturating_sub(1) as usize)
                .copied()
                .unwrap_or(0.0)
        };
        let lod_max_distance = if lod_index.saturating_add(1) >= lod_count {
            settings.cull.max_distance
        } else {
            settings
                .lod
                .mesh_distances
                .get(lod_index as usize)
                .copied()
                .unwrap_or(settings.cull.max_distance)
                .min(settings.cull.max_distance)
        };
        Self {
            lod_index,
            lod_count,
            lod_min_distance,
            lod_max_distance,
            cull_max_distance: settings.cull.max_distance,
            shadow_cull_max_distance: settings.cull.shadow_max_distance,
            wind_enabled: settings.wind.enabled,
            wind_strength: settings.wind.strength,
            wind_direction: settings.wind.direction,
        }
    }

    #[inline]
    pub fn is_visible(&self, distance: f32, shadow_pass: bool) -> bool {
        let cull_max_distance = if shadow_pass {
            self.shadow_cull_max_distance
        } else {
            self.cull_max_distance
        };
        distance >= self.lod_min_distance
            && distance <= self.lod_max_distance.min(cull_max_distance)
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoliageExtractionPathV1 {
    CpuFallback,
    GpuIndirect,
}

impl Default for FoliageExtractionPathV1 {
    fn default() -> Self {
        Self::CpuFallback
    }
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageExtractionRequestV1 {
    pub settings: FoliageSettings,
    pub runtime_asset: FoliageRuntimeAssetV1,
    pub instances: Vec<FoliageInstanceInputV1>,
    pub view: FoliageExtractionViewV1,
    pub capabilities: FoliageExtractionCapabilitiesV1,
}

impl Default for FoliageExtractionRequestV1 {
    fn default() -> Self {
        Self {
            settings: FoliageSettings::default(),
            runtime_asset: FoliageRuntimeAssetV1::default(),
            instances: Vec::new(),
            view: FoliageExtractionViewV1::default(),
            capabilities: FoliageExtractionCapabilitiesV1::default(),
        }
    }
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

/// Build renderer-facing foliage commands without mutating scene/ECS state.
///
/// GPU culling is selected only when both advertised capabilities are present.
/// Otherwise the same request is resolved by the deterministic CPU fallback.
pub fn build_foliage_extraction_plan_v1(
    request: FoliageExtractionRequestV1,
) -> Result<FoliageExtractionPlanV1, String> {
    let settings = request.settings.sanitized()?;
    request.runtime_asset.validate()?;

    let use_gpu = settings.prefer_gpu_culling
        && request.capabilities.gpu_culling
        && request.capabilities.indirect_draw;
    let path = if use_gpu {
        FoliageExtractionPathV1::GpuIndirect
    } else {
        FoliageExtractionPathV1::CpuFallback
    };

    let mut instances = request.instances;
    instances.sort_by_key(|instance| instance.stable_id);
    let input_instances = instances.len().min(u32::MAX as usize) as u32;
    let mut density_rejected = 0u32;
    let mut distance_culled = 0u32;
    let mut gpu_candidates = Vec::new();
    let mut batches = BTreeMap::<(u16, String, u64), Vec<FoliageInstanceCommandV1>>::new();

    for instance in instances {
        if density_fraction(instance.stable_id, settings.seed) > settings.density {
            density_rejected = density_rejected.saturating_add(1);
            continue;
        }

        let material_variant = instance
            .material_variant
            .as_deref()
            .unwrap_or(&settings.material_variant);
        let material = request.runtime_asset.material_for_variant(material_variant);
        if !material.is_valid() {
            return Err(format!(
                "foliage material variant '{}' has no registry-backed material handle",
                material_variant
            ));
        }

        if use_gpu {
            gpu_candidates.push(FoliageGpuCandidateV1 {
                stable_id: instance.stable_id,
                transform_cols: instance.transform_cols,
                bounds_center: instance.bounds_center,
                bounds_radius: instance.bounds_radius.abs().max(0.001),
                material,
            });
            continue;
        }

        let distance = instance_distance(&instance, request.view.camera_position);
        let radius_world = instance_world_radius(&instance);
        if distance - radius_world > settings.max_distance(request.view.shadow_pass) {
            distance_culled = distance_culled.saturating_add(1);
            continue;
        }

        let lod_index = settings.selected_lod(
            distance,
            request.runtime_asset.lods.len().min(u16::MAX as usize) as u16,
        );
        let lod = request
            .runtime_asset
            .lods
            .iter()
            .find(|lod| lod.lod_index == lod_index)
            .or_else(|| request.runtime_asset.lods.last())
            .expect("validated runtime asset has at least one LOD");
        let fade = lod_fade(distance, lod.max_distance, settings.lod.crossfade_width);
        let key = (lod.lod_index, lod.drawable_ref.clone(), material.raw());
        batches
            .entry(key)
            .or_default()
            .push(FoliageInstanceCommandV1 {
                stable_id: instance.stable_id,
                transform_cols: instance.transform_cols,
                lod_index: lod.lod_index,
                lod_fade: fade,
            });
    }

    let batches = batches
        .into_iter()
        .map(
            |((_lod_index, drawable_ref, material), instances)| FoliageDrawBatchV1 {
                drawable_ref,
                material: MaterialId(material),
                instances,
            },
        )
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    if use_gpu {
        warnings.push(format!(
            "GPU foliage candidates require capability '{}'; CPU fallback remains available",
            FOLIAGE_GPU_CULLING_CAPABILITY_ID
        ));
    }

    let gpu_work = use_gpu.then(|| FoliageGpuWorkV1 {
        settings: settings.clone(),
        lods: request.runtime_asset.lods.clone(),
        view: request.view,
        candidates: gpu_candidates,
    });

    Ok(FoliageExtractionPlanV1 {
        schema: FOLIAGE_EXTRACTION_PLAN_SCHEMA.to_owned(),
        path,
        wind: settings.wind,
        batches,
        gpu_work,
        input_instances,
        density_rejected,
        distance_culled,
        warnings,
    })
}

fn normalize_logical_source(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned()
}

fn has_extension(value: &str, extension: &str) -> bool {
    value
        .rsplit_once('.')
        .map(|(_, actual)| actual.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

fn speedtree_importer_id(value: &str) -> Option<&'static str> {
    if has_extension(value, SPEEDTREE_SRT_EXTENSION) {
        Some(FOLIAGE_SRT_IMPORTER_ID)
    } else if has_extension(value, SPEEDTREE_SPM_EXTENSION) {
        Some(FOLIAGE_SPM_IMPORTER_ID)
    } else {
        None
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn normalized_direction(direction: [f32; 3]) -> [f32; 3] {
    let x = finite_or(direction[0], 0.78);
    let y = finite_or(direction[1], 0.0);
    let z = finite_or(direction[2], 0.62);
    let length_sq = x * x + y * y + z * z;
    if length_sq <= 1.0e-8 {
        return FoliageWindSettings::default().direction;
    }
    let inv_len = length_sq.sqrt().recip();
    [x * inv_len, y * inv_len, z * inv_len]
}

fn sanitize_lod_distances(values: Vec<f32>) -> Vec<f32> {
    let mut values = values
        .into_iter()
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.25, 16_384.0))
        .collect::<Vec<_>>();
    values.sort_by(f32::total_cmp);
    values.dedup_by(|left, right| (*left - *right).abs() <= 0.001);
    if values.is_empty() {
        FoliageLodSettings::default().mesh_distances
    } else {
        values
    }
}

fn identity_cols() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn density_fraction(stable_id: u64, seed: u64) -> f32 {
    let mut value = stable_id ^ seed ^ 0x9E37_79B9_7F4A_7C15;
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    ((value >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

fn instance_distance(instance: &FoliageInstanceInputV1, camera: [f32; 3]) -> f32 {
    let center = instance_world_center(instance);
    let dx = center[0] - camera[0];
    let dy = center[1] - camera[1];
    let dz = center[2] - camera[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn instance_world_center(instance: &FoliageInstanceInputV1) -> [f32; 3] {
    let cols = instance.transform_cols;
    let local = instance.bounds_center;
    [
        cols[3][0] + cols[0][0] * local[0] + cols[1][0] * local[1] + cols[2][0] * local[2],
        cols[3][1] + cols[0][1] * local[0] + cols[1][1] * local[1] + cols[2][1] * local[2],
        cols[3][2] + cols[0][2] * local[0] + cols[1][2] * local[1] + cols[2][2] * local[2],
    ]
}

fn instance_world_radius(instance: &FoliageInstanceInputV1) -> f32 {
    let axis_length = |column: [f32; 4]| {
        (column[0] * column[0] + column[1] * column[1] + column[2] * column[2]).sqrt()
    };
    let max_scale = axis_length(instance.transform_cols[0])
        .max(axis_length(instance.transform_cols[1]))
        .max(axis_length(instance.transform_cols[2]))
        .max(0.001);
    instance.bounds_radius.abs().max(0.001) * max_scale
}

fn lod_fade(distance: f32, max_distance: f32, width: f32) -> f32 {
    if !max_distance.is_finite() || width <= f32::EPSILON {
        return 1.0;
    }
    ((max_distance - distance) / width).clamp(0.0, 1.0)
}
#[cfg(test)]
#[path = "foliage/tests.rs"]
mod tests;
