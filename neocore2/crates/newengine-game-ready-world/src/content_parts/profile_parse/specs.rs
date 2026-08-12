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
    #[serde(default = "default_foliage_prefab")]
    pub(in super::super) prefab: String,
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
}

impl Default for RawFoliageSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            prefab: default_foliage_prefab(),
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
