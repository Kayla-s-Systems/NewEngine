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
