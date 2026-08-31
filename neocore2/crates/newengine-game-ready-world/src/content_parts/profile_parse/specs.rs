use super::*;

#[derive(Debug, Deserialize)]
pub(in super::super) struct RawShadowSpec {
    #[serde(default = "default_shadow_enabled")]
    pub(in super::super) enabled: bool,
    #[serde(default = "default_shadow_resolution")]
    pub(in super::super) resolution: u32,
    #[serde(default = "default_shadow_cascade_count")]
    pub(in super::super) cascade_count: u32,
    #[serde(default = "default_shadow_max_distance")]
    pub(in super::super) max_distance: f32,
    #[serde(default = "default_shadow_softness")]
    pub(in super::super) softness: f32,
    #[serde(default = "default_shadow_bias")]
    pub(in super::super) bias: f32,
    #[serde(default = "default_shadow_normal_bias")]
    pub(in super::super) normal_bias: f32,
    #[serde(default = "default_shadow_contact_strength")]
    pub(in super::super) contact_strength: f32,
    #[serde(default = "default_shadow_filter")]
    pub(in super::super) filter: String,
    #[serde(default = "default_shadow_pcss_light_angular_radius_degrees")]
    pub(in super::super) pcss_light_angular_radius_degrees: f32,
    #[serde(default = "default_shadow_pcss_blocker_search_radius_texels")]
    pub(in super::super) pcss_blocker_search_radius_texels: f32,
    #[serde(default = "default_shadow_pcss_max_filter_radius_texels")]
    pub(in super::super) pcss_max_filter_radius_texels: f32,
    #[serde(default = "default_shadow_pcss_blocker_samples")]
    pub(in super::super) pcss_blocker_samples: u32,
    #[serde(default = "default_shadow_pcss_filter_samples")]
    pub(in super::super) pcss_filter_samples: u32,
    #[serde(default = "default_shadow_pcss_min_filter_radius_texels")]
    pub(in super::super) pcss_min_filter_radius_texels: f32,
    #[serde(default = "default_shadow_pcss_stable_kernel_cell_texels")]
    pub(in super::super) pcss_stable_kernel_cell_texels: f32,
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
            filter: default_shadow_filter(),
            pcss_light_angular_radius_degrees: default_shadow_pcss_light_angular_radius_degrees(),
            pcss_blocker_search_radius_texels: default_shadow_pcss_blocker_search_radius_texels(),
            pcss_max_filter_radius_texels: default_shadow_pcss_max_filter_radius_texels(),
            pcss_blocker_samples: default_shadow_pcss_blocker_samples(),
            pcss_filter_samples: default_shadow_pcss_filter_samples(),
            pcss_min_filter_radius_texels: default_shadow_pcss_min_filter_radius_texels(),
            pcss_stable_kernel_cell_texels: default_shadow_pcss_stable_kernel_cell_texels(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct RawFoliageSpec {
    #[serde(default)]
    pub(in super::super) enabled: bool,
    /// Optional opaque SpeedTree SRT/SPM source. The prefab remains the compiled YDD runtime output.
    #[serde(
        default,
        alias = "srt",
        alias = "source_srt",
        alias = "spm",
        alias = "source_spm"
    )]
    pub(in super::super) canonical_path: String,
    #[serde(default)]
    pub(in super::super) density: Option<f32>,
    #[serde(default)]
    pub(in super::super) material_variant: String,
    #[serde(default)]
    pub(in super::super) wind_enabled: Option<bool>,
    #[serde(default)]
    pub(in super::super) wind_strength: Option<f32>,
    #[serde(default)]
    pub(in super::super) wind_gust_frequency: Option<f32>,
    #[serde(default)]
    pub(in super::super) wind_direction_x: Option<f32>,
    #[serde(default)]
    pub(in super::super) wind_direction_y: Option<f32>,
    #[serde(default)]
    pub(in super::super) wind_direction_z: Option<f32>,
    #[serde(default)]
    pub(in super::super) lod0_distance: Option<f32>,
    #[serde(default)]
    pub(in super::super) lod1_distance: Option<f32>,
    #[serde(default)]
    pub(in super::super) lod2_distance: Option<f32>,
    #[serde(default)]
    pub(in super::super) impostor_distance: Option<f32>,
    #[serde(default)]
    pub(in super::super) lod_crossfade_width: Option<f32>,
    #[serde(default)]
    pub(in super::super) cull_distance: Option<f32>,
    #[serde(default)]
    pub(in super::super) shadow_cull_distance: Option<f32>,
    #[serde(default)]
    pub(in super::super) prefer_gpu_culling: Option<bool>,
    #[serde(default = "default_foliage_prefab")]
    pub(in super::super) prefab: String,
    /// Optional second compiled foliage prefab mixed into the same placement set.
    #[serde(default)]
    pub(in super::super) alternate_prefab: String,
    /// Optional source ref used for provider/import diagnostics for the alternate prefab.
    #[serde(default)]
    pub(in super::super) alternate_canonical_path: String,
    #[serde(default)]
    pub(in super::super) alternate_weight: Option<f32>,
    #[serde(default)]
    pub(in super::super) alternate_collision_radius: Option<f32>,
    #[serde(default)]
    pub(in super::super) alternate_collision_half_height: Option<f32>,
    #[serde(default)]
    pub(in super::super) alternate_collision_center_x: Option<f32>,
    #[serde(default)]
    pub(in super::super) alternate_collision_center_y: Option<f32>,
    #[serde(default)]
    pub(in super::super) alternate_collision_center_z: Option<f32>,
    #[serde(default = "default_foliage_seed")]
    pub(in super::super) seed: u64,
    #[serde(default = "default_foliage_grid_min")]
    pub(in super::super) grid_min: i32,
    #[serde(default = "default_foliage_grid_max")]
    pub(in super::super) grid_max: i32,
    #[serde(default = "default_foliage_spacing")]
    pub(in super::super) spacing: f32,
    #[serde(default = "default_foliage_jitter")]
    pub(in super::super) jitter: f32,
    #[serde(default = "default_foliage_gate_threshold")]
    pub(in super::super) gate_threshold: f32,
    #[serde(default)]
    pub(in super::super) max_count: u32,
    #[serde(default = "default_foliage_min_scale")]
    pub(in super::super) min_scale: f32,
    #[serde(default = "default_foliage_max_scale")]
    pub(in super::super) max_scale: f32,
    #[serde(default = "default_foliage_min_player_distance")]
    pub(in super::super) min_player_distance: f32,
    #[serde(default = "default_foliage_edge_margin")]
    pub(in super::super) edge_margin: f32,
    #[serde(default = "default_foliage_surface_offset")]
    pub(in super::super) surface_offset: f32,
    /// Optional static trunk collision proxy. Values are authored in metres in
    /// the compiled tree's local space and scaled per foliage placement.
    #[serde(default)]
    pub(in super::super) collision_enabled: bool,
    #[serde(default)]
    pub(in super::super) collision_radius: Option<f32>,
    #[serde(default)]
    pub(in super::super) collision_half_height: Option<f32>,
    #[serde(default)]
    pub(in super::super) collision_center_x: Option<f32>,
    #[serde(default)]
    pub(in super::super) collision_center_y: Option<f32>,
    #[serde(default)]
    pub(in super::super) collision_center_z: Option<f32>,
}

impl Default for RawFoliageSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            canonical_path: String::new(),
            density: None,
            material_variant: String::new(),
            wind_enabled: None,
            wind_strength: None,
            wind_gust_frequency: None,
            wind_direction_x: None,
            wind_direction_y: None,
            wind_direction_z: None,
            lod0_distance: None,
            lod1_distance: None,
            lod2_distance: None,
            impostor_distance: None,
            lod_crossfade_width: None,
            cull_distance: None,
            shadow_cull_distance: None,
            prefer_gpu_culling: None,
            prefab: default_foliage_prefab(),
            alternate_prefab: String::new(),
            alternate_canonical_path: String::new(),
            alternate_weight: None,
            alternate_collision_radius: None,
            alternate_collision_half_height: None,
            alternate_collision_center_x: None,
            alternate_collision_center_y: None,
            alternate_collision_center_z: None,
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
            collision_enabled: false,
            collision_radius: None,
            collision_half_height: None,
            collision_center_x: None,
            collision_center_y: None,
            collision_center_z: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct RawPrefabSpec {
    #[serde(default)]
    pub(in super::super) id: String,
    #[serde(default)]
    pub(in super::super) source: String,
    #[serde(default = "default_prefab_proxy")]
    pub(in super::super) proxy: String,
    /// Optional exact NEMAT selector used by static world geometry.
    #[serde(default)]
    pub(in super::super) material: String,
    /// Opaque project-authored surface id. Runtime never infers this from material/file names.
    #[serde(default)]
    pub(in super::super) surface_id: String,
    /// Generic surface signal -> arbitrary project gameplay event id.
    #[serde(default)]
    pub(in super::super) surface_events: std::collections::BTreeMap<String, String>,
    /// Explicit capability consumed by foliage placement; never inferred from prefab id/material.
    #[serde(default)]
    pub(in super::super) ground_placement_surface: bool,
    #[serde(default = "default_prefab_enabled")]
    pub(in super::super) enabled: bool,
    #[serde(default)]
    pub(in super::super) position: [f32; 3],
    #[serde(default)]
    pub(in super::super) rotation_ypr: [f32; 3],
    #[serde(default = "default_definition_scale")]
    pub(in super::super) scale: [f32; 3],
}

#[derive(Debug, Deserialize)]
pub(in super::super) struct RawDefinitionInstanceSpec {
    #[serde(default)]
    pub(in super::super) definition_ref: String,
    /// Declarative apply behavior for this `.ymap` definition placement.
    /// Default is metadata-only so `.ytyp` dependencies remain graph inputs,
    /// not implicit render/spawn commands.
    #[serde(default = "default_definition_apply_mode")]
    pub(in super::super) apply_mode: String,
    #[serde(default)]
    pub(in super::super) position: [f32; 3],
    #[serde(default)]
    pub(in super::super) rotation_ypr: [f32; 3],
    #[serde(default = "default_definition_scale")]
    pub(in super::super) scale: [f32; 3],
}
