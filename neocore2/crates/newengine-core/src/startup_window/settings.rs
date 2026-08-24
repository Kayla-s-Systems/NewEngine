#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

pub const STARTUP_SETTINGS_SCHEMA_VERSION: u32 = 3;

pub const ENV_GRAPHICS_PRESET: &str = "NEWENGINE_GRAPHICS_PRESET";
pub const ENV_RENDER_SCALE: &str = "NEWENGINE_GRAPHICS_RENDER_SCALE";
pub const ENV_MSAA_SAMPLES: &str = "NEWENGINE_GRAPHICS_MSAA_SAMPLES";
pub const ENV_FXAA_ENABLED: &str = "NEWENGINE_GRAPHICS_FXAA_ENABLED";
pub const ENV_FXAA_EDGE_THRESHOLD: &str = "NEWENGINE_GRAPHICS_FXAA_EDGE_THRESHOLD";
pub const ENV_FXAA_EDGE_THRESHOLD_MIN: &str = "NEWENGINE_GRAPHICS_FXAA_EDGE_THRESHOLD_MIN";
pub const ENV_FXAA_SUBPIXEL_QUALITY: &str = "NEWENGINE_GRAPHICS_FXAA_SUBPIXEL_QUALITY";
pub const ENV_TAA_ENABLED: &str = "NEWENGINE_GRAPHICS_TAA_ENABLED";
pub const ENV_TAA_FEEDBACK: &str = "NEWENGINE_GRAPHICS_TAA_FEEDBACK";
pub const ENV_TAA_NEIGHBORHOOD_CLAMPING: &str = "NEWENGINE_GRAPHICS_TAA_NEIGHBORHOOD_CLAMPING";
pub const ENV_TAA_JITTER_SCALE: &str = "NEWENGINE_GRAPHICS_TAA_JITTER_SCALE";
pub const ENV_SSAO_ENABLED: &str = "NEWENGINE_GRAPHICS_SSAO_ENABLED";
pub const ENV_SSAO_RADIUS_WS: &str = "NEWENGINE_GRAPHICS_SSAO_RADIUS_WS";
pub const ENV_SSAO_INTENSITY: &str = "NEWENGINE_GRAPHICS_SSAO_INTENSITY";
pub const ENV_SSAO_QUALITY_STEPS: &str = "NEWENGINE_GRAPHICS_SSAO_QUALITY_STEPS";
pub const ENV_SSAO_HALF_RESOLUTION: &str = "NEWENGINE_GRAPHICS_SSAO_HALF_RESOLUTION";
pub const ENV_BLOOM_ENABLED: &str = "NEWENGINE_GRAPHICS_BLOOM_ENABLED";
pub const ENV_BLOOM_THRESHOLD: &str = "NEWENGINE_GRAPHICS_BLOOM_THRESHOLD";
pub const ENV_BLOOM_KNEE: &str = "NEWENGINE_GRAPHICS_BLOOM_KNEE";
pub const ENV_BLOOM_INTENSITY: &str = "NEWENGINE_GRAPHICS_BLOOM_INTENSITY";
pub const ENV_BLOOM_RADIUS: &str = "NEWENGINE_GRAPHICS_BLOOM_RADIUS";
pub const ENV_DOF_ENABLED: &str = "NEWENGINE_GRAPHICS_DOF_ENABLED";
pub const ENV_MOTION_BLUR_ENABLED: &str = "NEWENGINE_GRAPHICS_MOTION_BLUR_ENABLED";
pub const ENV_SUN_RAYS_ENABLED: &str = "NEWENGINE_GRAPHICS_SUN_RAYS_ENABLED";
pub const ENV_SHADOWS_ENABLED: &str = "NEWENGINE_GRAPHICS_SHADOWS_ENABLED";
pub const ENV_SHADOW_QUALITY: &str = "NEWENGINE_GRAPHICS_SHADOW_QUALITY";
pub const ENV_SHADOW_CASCADE_COUNT: &str = "NEWENGINE_GRAPHICS_SHADOW_CASCADE_COUNT";
pub const ENV_SHADOW_MAP_RESOLUTION: &str = "NEWENGINE_GRAPHICS_SHADOW_MAP_RESOLUTION";
pub const ENV_SHADOW_FILTER: &str = "NEWENGINE_GRAPHICS_SHADOW_FILTER";
pub const ENV_SHADOW_MAX_DISTANCE: &str = "NEWENGINE_GRAPHICS_SHADOW_MAX_DISTANCE";
pub const ENV_SHADOW_SOFTNESS: &str = "NEWENGINE_GRAPHICS_SHADOW_SOFTNESS";
pub const ENV_SHADOW_BIAS: &str = "NEWENGINE_GRAPHICS_SHADOW_BIAS";
pub const ENV_SHADOW_NORMAL_BIAS: &str = "NEWENGINE_GRAPHICS_SHADOW_NORMAL_BIAS";
pub const ENV_SHADOW_CONTACT_STRENGTH: &str = "NEWENGINE_GRAPHICS_SHADOW_CONTACT_STRENGTH";
pub const ENV_SHADOW_PCSS_LIGHT_RADIUS_DEGREES: &str =
    "NEWENGINE_GRAPHICS_SHADOW_PCSS_LIGHT_RADIUS_DEGREES";
pub const ENV_SHADOW_PCSS_BLOCKER_RADIUS_TEXELS: &str =
    "NEWENGINE_GRAPHICS_SHADOW_PCSS_BLOCKER_RADIUS_TEXELS";
pub const ENV_SHADOW_PCSS_MAX_FILTER_RADIUS_TEXELS: &str =
    "NEWENGINE_GRAPHICS_SHADOW_PCSS_MAX_FILTER_RADIUS_TEXELS";
pub const ENV_SHADOW_PCSS_BLOCKER_SAMPLES: &str = "NEWENGINE_GRAPHICS_SHADOW_PCSS_BLOCKER_SAMPLES";
pub const ENV_SHADOW_PCSS_FILTER_SAMPLES: &str = "NEWENGINE_GRAPHICS_SHADOW_PCSS_FILTER_SAMPLES";
pub const ENV_SHADOW_PCSS_MIN_FILTER_RADIUS_TEXELS: &str =
    "NEWENGINE_GRAPHICS_SHADOW_PCSS_MIN_FILTER_RADIUS_TEXELS";
pub const ENV_SHADOW_PCSS_STABLE_KERNEL_TEXELS: &str =
    "NEWENGINE_GRAPHICS_SHADOW_PCSS_STABLE_KERNEL_TEXELS";
pub const ENV_LOD_QUALITY: &str = "NEWENGINE_GRAPHICS_LOD_QUALITY";
pub const ENV_LOD_DISTANCE_SCALE: &str = "NEWENGINE_GRAPHICS_LOD_DISTANCE_SCALE";
pub const ENV_TEXTURE_QUALITY: &str = "NEWENGINE_GRAPHICS_TEXTURE_QUALITY";
pub const ENV_ANISOTROPY: &str = "NEWENGINE_GRAPHICS_ANISOTROPY";
pub const ENV_WINDOW_MODE: &str = "NEWENGINE_DISPLAY_WINDOW_MODE";
pub const ENV_VSYNC: &str = "NEWENGINE_DISPLAY_VSYNC";
pub const ENV_REFRESH_RATE_MILLIHZ: &str = "NEWENGINE_DISPLAY_REFRESH_RATE_MILLIHZ";
pub const ENV_HDR_MODE: &str = "NEWENGINE_DISPLAY_HDR_MODE";
pub const ENV_FRAME_LIMIT: &str = "NEWENGINE_DISPLAY_FRAME_LIMIT";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsPreset {
    Low,
    Balanced,
    High,
    Ultra,
    Custom,
}

impl GraphicsPreset {
    pub const ALL: [Self; 5] = [
        Self::Low,
        Self::Balanced,
        Self::High,
        Self::Ultra,
        Self::Custom,
    ];

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Balanced => "balanced",
            Self::High => "high",
            Self::Ultra => "ultra",
            Self::Custom => "custom",
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Balanced => "Balanced",
            Self::High => "High",
            Self::Ultra => "Ultra",
            Self::Custom => "Custom",
        }
    }
}

impl Default for GraphicsPreset {
    #[inline]
    fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowQuality {
    Off,
    Performance,
    Balanced,
    Quality,
    Cinematic,
}

impl ShadowQuality {
    pub const ALL: [Self; 5] = [
        Self::Off,
        Self::Performance,
        Self::Balanced,
        Self::Quality,
        Self::Cinematic,
    ];

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Performance => "performance",
            Self::Balanced => "balanced",
            Self::Quality => "quality",
            Self::Cinematic => "cinematic",
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Performance => "Performance",
            Self::Balanced => "Balanced",
            Self::Quality => "Quality",
            Self::Cinematic => "Cinematic",
        }
    }
}

impl Default for ShadowQuality {
    #[inline]
    fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowFilterMode {
    Hard,
    Pcf,
    Pcss,
}

impl ShadowFilterMode {
    pub const ALL: [Self; 3] = [Self::Hard, Self::Pcf, Self::Pcss];
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hard => "hard",
            Self::Pcf => "pcf",
            Self::Pcss => "pcss",
        }
    }
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hard => "Hard",
            Self::Pcf => "PCF",
            Self::Pcss => "PCSS",
        }
    }
}
impl Default for ShadowFilterMode {
    fn default() -> Self {
        Self::Pcss
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LodQuality {
    Low,
    Medium,
    High,
    Ultra,
    Cinematic,
    Custom,
}

impl LodQuality {
    pub const ALL: [Self; 6] = [
        Self::Low,
        Self::Medium,
        Self::High,
        Self::Ultra,
        Self::Cinematic,
        Self::Custom,
    ];
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
            Self::Cinematic => "cinematic",
            Self::Custom => "custom",
        }
    }
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Ultra => "Ultra",
            Self::Cinematic => "Cinematic",
            Self::Custom => "Custom",
        }
    }
    #[inline]
    pub const fn distance_scale(self) -> Option<f32> {
        match self {
            Self::Low => Some(0.65),
            Self::Medium => Some(0.85),
            Self::High => Some(1.0),
            Self::Ultra => Some(1.35),
            Self::Cinematic => Some(1.75),
            Self::Custom => None,
        }
    }
}
impl Default for LodQuality {
    fn default() -> Self {
        Self::High
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureQuality {
    Low,
    Medium,
    High,
    Ultra,
}

impl TextureQuality {
    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Ultra => "Ultra",
        }
    }
}

impl Default for TextureQuality {
    #[inline]
    fn default() -> Self {
        Self::High
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupWindowMode {
    Windowed,
    Borderless,
    ExclusiveFullscreen,
}

impl StartupWindowMode {
    pub const ALL: [Self; 3] = [Self::Windowed, Self::Borderless, Self::ExclusiveFullscreen];

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windowed => "windowed",
            Self::Borderless => "borderless",
            Self::ExclusiveFullscreen => "exclusive_fullscreen",
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Windowed => "Windowed",
            Self::Borderless => "Borderless fullscreen",
            Self::ExclusiveFullscreen => "Exclusive fullscreen",
        }
    }
}

impl Default for StartupWindowMode {
    #[inline]
    fn default() -> Self {
        Self::Windowed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupHdrMode {
    Auto,
    Enabled,
    Disabled,
}

impl StartupHdrMode {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Enabled, Self::Disabled];

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Enabled => "Enabled",
            Self::Disabled => "Disabled",
        }
    }
}

impl Default for StartupHdrMode {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupDisplaySettings {
    pub monitor_index: i32,
    pub window_mode: StartupWindowMode,
    pub vsync: bool,
    pub refresh_rate_millihz: u32,
    pub render_scale: f32,
    pub hdr: StartupHdrMode,
    /// 0 means uncapped. The active platform/runtime may clamp further.
    pub frame_limit: u32,
    pub center_window: bool,
}

impl Default for StartupDisplaySettings {
    fn default() -> Self {
        Self {
            monitor_index: -1,
            window_mode: StartupWindowMode::Windowed,
            vsync: true,
            refresh_rate_millihz: 0,
            render_scale: 1.0,
            hdr: StartupHdrMode::Auto,
            frame_limit: 0,
            center_window: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupGraphicsSettings {
    pub preset: GraphicsPreset,
    pub msaa_samples: u8,
    pub fxaa_enabled: bool,
    pub fxaa_edge_threshold: f32,
    pub fxaa_edge_threshold_min: f32,
    pub fxaa_subpixel_quality: f32,
    pub taa_enabled: bool,
    pub taa_feedback: f32,
    pub taa_neighborhood_clamping: f32,
    pub taa_jitter_scale: f32,
    pub ssao_enabled: bool,
    pub ssao_radius_ws: f32,
    pub ssao_intensity: f32,
    pub ssao_quality_steps: u32,
    pub ssao_half_resolution: bool,
    pub bloom_enabled: bool,
    pub bloom_threshold: f32,
    pub bloom_knee: f32,
    pub bloom_intensity: f32,
    pub bloom_radius: f32,
    pub depth_of_field_enabled: bool,
    pub motion_blur_enabled: bool,
    pub sun_rays_enabled: bool,
    pub shadows_enabled: bool,
    pub shadow_quality: ShadowQuality,
    /// 0 keeps the scene-authored cascade count; 1..=4 overrides it for this launch.
    pub shadow_cascade_count: u32,
    /// 0 keeps the scene-authored map size; otherwise one of the supported 256..=16284 launch overrides.
    pub shadow_map_resolution: u32,
    pub shadow_advanced_override: bool,
    pub shadow_filter: ShadowFilterMode,
    pub shadow_max_distance: f32,
    pub shadow_softness: f32,
    pub shadow_bias: f32,
    pub shadow_normal_bias: f32,
    pub shadow_contact_strength: f32,
    pub shadow_pcss_light_radius_degrees: f32,
    pub shadow_pcss_blocker_radius_texels: f32,
    pub shadow_pcss_max_filter_radius_texels: f32,
    pub shadow_pcss_blocker_samples: u32,
    pub shadow_pcss_filter_samples: u32,
    pub shadow_pcss_min_filter_radius_texels: f32,
    pub shadow_pcss_stable_kernel_texels: f32,
    pub lod_quality: LodQuality,
    /// Global distance multiplier used by runtime visibility/LOD policy. 1.0 preserves authored/default distances.
    pub lod_distance_scale: f32,
    pub texture_quality: TextureQuality,
    pub anisotropy: u8,
}

impl Default for StartupGraphicsSettings {
    fn default() -> Self {
        let value = Self {
            preset: GraphicsPreset::Balanced,
            msaa_samples: 0,
            fxaa_enabled: true,
            fxaa_edge_threshold: 0.125,
            fxaa_edge_threshold_min: 0.0312,
            fxaa_subpixel_quality: 0.75,
            taa_enabled: false,
            taa_feedback: 0.92,
            taa_neighborhood_clamping: 1.0,
            taa_jitter_scale: 1.0,
            ssao_enabled: false,
            ssao_radius_ws: 0.75,
            ssao_intensity: 0.82,
            ssao_quality_steps: 16,
            ssao_half_resolution: true,
            bloom_enabled: true,
            bloom_threshold: 0.85,
            bloom_knee: 0.35,
            bloom_intensity: 0.085,
            bloom_radius: 1.0,
            depth_of_field_enabled: false,
            motion_blur_enabled: false,
            sun_rays_enabled: true,
            shadows_enabled: true,
            shadow_quality: ShadowQuality::Balanced,
            shadow_cascade_count: 0,
            shadow_map_resolution: 0,
            shadow_advanced_override: false,
            shadow_filter: ShadowFilterMode::Pcss,
            shadow_max_distance: 80.0,
            shadow_softness: 1.0,
            shadow_bias: 0.0025,
            shadow_normal_bias: 0.015,
            shadow_contact_strength: 0.25,
            shadow_pcss_light_radius_degrees: 0.266,
            shadow_pcss_blocker_radius_texels: 3.0,
            shadow_pcss_max_filter_radius_texels: 5.0,
            shadow_pcss_blocker_samples: 10,
            shadow_pcss_filter_samples: 12,
            shadow_pcss_min_filter_radius_texels: 0.18,
            shadow_pcss_stable_kernel_texels: 8.0,
            lod_quality: LodQuality::High,
            lod_distance_scale: 1.0,
            texture_quality: TextureQuality::High,
            anisotropy: 8,
        };
        // Default launch settings preserve scene-authored cascade/map topology. Quality
        // presets become authoritative only when the user explicitly selects one.
        value
    }
}

impl StartupGraphicsSettings {
    pub fn apply_preset(&mut self, preset: GraphicsPreset) {
        self.preset = preset;
        match preset {
            GraphicsPreset::Low => {
                self.shadow_advanced_override = false;
                self.msaa_samples = 0;
                self.fxaa_enabled = true;
                self.taa_enabled = false;
                self.ssao_enabled = false;
                self.ssao_quality_steps = 8;
                self.ssao_half_resolution = true;
                self.bloom_enabled = false;
                self.depth_of_field_enabled = false;
                self.motion_blur_enabled = false;
                self.sun_rays_enabled = false;
                self.shadows_enabled = true;
                self.shadow_quality = ShadowQuality::Performance;
                self.shadow_cascade_count = 2;
                self.shadow_map_resolution = 512;
                self.lod_distance_scale = 0.65;
                self.texture_quality = TextureQuality::Low;
                self.anisotropy = 2;
                self.shadow_filter = ShadowFilterMode::Pcf;
                self.shadow_max_distance = 48.0;
                self.shadow_softness = 0.7;
                self.shadow_bias = 0.0025;
                self.shadow_normal_bias = 0.015;
                self.shadow_contact_strength = 0.10;
                self.shadow_pcss_light_radius_degrees = 0.266;
                self.shadow_pcss_blocker_radius_texels = 2.0;
                self.shadow_pcss_max_filter_radius_texels = 3.0;
                self.shadow_pcss_blocker_samples = 6;
                self.shadow_pcss_filter_samples = 8;
                self.shadow_pcss_min_filter_radius_texels = 0.18;
                self.shadow_pcss_stable_kernel_texels = 8.0;
                self.lod_quality = LodQuality::Low;
            }
            GraphicsPreset::Balanced => {
                self.shadow_advanced_override = false;
                self.msaa_samples = 0;
                self.fxaa_enabled = true;
                self.taa_enabled = false;
                self.ssao_enabled = false;
                self.ssao_quality_steps = 16;
                self.ssao_half_resolution = true;
                self.bloom_enabled = true;
                self.depth_of_field_enabled = false;
                self.motion_blur_enabled = false;
                self.sun_rays_enabled = true;
                self.shadows_enabled = true;
                self.shadow_quality = ShadowQuality::Balanced;
                self.shadow_cascade_count = 3;
                self.shadow_map_resolution = 1024;
                self.lod_distance_scale = 0.85;
                self.texture_quality = TextureQuality::High;
                self.anisotropy = 8;
                self.shadow_filter = ShadowFilterMode::Pcss;
                self.shadow_max_distance = 80.0;
                self.shadow_softness = 1.0;
                self.shadow_bias = 0.0025;
                self.shadow_normal_bias = 0.015;
                self.shadow_contact_strength = 0.25;
                self.shadow_pcss_light_radius_degrees = 0.266;
                self.shadow_pcss_blocker_radius_texels = 3.0;
                self.shadow_pcss_max_filter_radius_texels = 5.0;
                self.shadow_pcss_blocker_samples = 8;
                self.shadow_pcss_filter_samples = 12;
                self.shadow_pcss_min_filter_radius_texels = 0.18;
                self.shadow_pcss_stable_kernel_texels = 8.0;
                self.lod_quality = LodQuality::Medium;
            }
            GraphicsPreset::High => {
                self.shadow_advanced_override = false;
                self.msaa_samples = 2;
                self.fxaa_enabled = true;
                self.taa_enabled = false;
                self.ssao_enabled = true;
                self.ssao_quality_steps = 24;
                self.ssao_half_resolution = true;
                self.bloom_enabled = true;
                self.depth_of_field_enabled = false;
                self.motion_blur_enabled = false;
                self.sun_rays_enabled = true;
                self.shadows_enabled = true;
                self.shadow_quality = ShadowQuality::Quality;
                self.shadow_cascade_count = 4;
                self.shadow_map_resolution = 2048;
                self.lod_distance_scale = 1.0;
                self.texture_quality = TextureQuality::High;
                self.anisotropy = 8;
                self.shadow_filter = ShadowFilterMode::Pcss;
                self.shadow_max_distance = 140.0;
                self.shadow_softness = 1.0;
                self.shadow_bias = 0.0025;
                self.shadow_normal_bias = 0.015;
                self.shadow_contact_strength = 0.25;
                self.shadow_pcss_light_radius_degrees = 0.266;
                self.shadow_pcss_blocker_radius_texels = 3.0;
                self.shadow_pcss_max_filter_radius_texels = 5.0;
                self.shadow_pcss_blocker_samples = 12;
                self.shadow_pcss_filter_samples = 16;
                self.shadow_pcss_min_filter_radius_texels = 0.18;
                self.shadow_pcss_stable_kernel_texels = 8.0;
                self.lod_quality = LodQuality::High;
            }
            GraphicsPreset::Ultra => {
                self.shadow_advanced_override = false;
                self.msaa_samples = 4;
                self.fxaa_enabled = true;
                self.taa_enabled = true;
                self.ssao_enabled = true;
                self.ssao_quality_steps = 32;
                self.ssao_half_resolution = false;
                self.bloom_enabled = true;
                self.depth_of_field_enabled = true;
                self.motion_blur_enabled = true;
                self.sun_rays_enabled = true;
                self.shadows_enabled = true;
                self.shadow_quality = ShadowQuality::Cinematic;
                self.shadow_cascade_count = 4;
                self.shadow_map_resolution = 4096;
                self.lod_distance_scale = 1.35;
                self.texture_quality = TextureQuality::Ultra;
                self.anisotropy = 16;
                self.shadow_filter = ShadowFilterMode::Pcss;
                self.shadow_max_distance = 240.0;
                self.shadow_softness = 1.0;
                self.shadow_bias = 0.0025;
                self.shadow_normal_bias = 0.015;
                self.shadow_contact_strength = 0.25;
                self.shadow_pcss_light_radius_degrees = 0.266;
                self.shadow_pcss_blocker_radius_texels = 3.0;
                self.shadow_pcss_max_filter_radius_texels = 8.0;
                self.shadow_pcss_blocker_samples = 16;
                self.shadow_pcss_filter_samples = 16;
                self.shadow_pcss_min_filter_radius_texels = 0.18;
                self.shadow_pcss_stable_kernel_texels = 8.0;
                self.lod_quality = LodQuality::Ultra;
            }
            GraphicsPreset::Custom => {}
        }
    }

    pub fn normalize(&mut self) {
        self.msaa_samples = match self.msaa_samples {
            2 | 4 | 8 => self.msaa_samples,
            _ => 0,
        };
        self.anisotropy = match self.anisotropy {
            2 | 4 | 8 | 16 => self.anisotropy,
            _ => 0,
        };
        self.fxaa_edge_threshold = self.fxaa_edge_threshold.clamp(0.01, 1.0);
        self.fxaa_edge_threshold_min = self.fxaa_edge_threshold_min.clamp(0.001, 1.0);
        self.fxaa_subpixel_quality = self.fxaa_subpixel_quality.clamp(0.0, 1.0);
        self.taa_feedback = self.taa_feedback.clamp(0.0, 0.99);
        self.taa_neighborhood_clamping = self.taa_neighborhood_clamping.clamp(0.0, 4.0);
        self.taa_jitter_scale = self.taa_jitter_scale.clamp(0.0, 2.0);
        self.ssao_radius_ws = self.ssao_radius_ws.clamp(0.05, 10.0);
        self.ssao_intensity = self.ssao_intensity.clamp(0.0, 4.0);
        self.ssao_quality_steps = self.ssao_quality_steps.clamp(4, 64);
        self.bloom_threshold = self.bloom_threshold.clamp(0.0, 20.0);
        self.bloom_knee = self.bloom_knee.clamp(0.0, 5.0);
        self.bloom_intensity = self.bloom_intensity.clamp(0.0, 5.0);
        self.bloom_radius = self.bloom_radius.clamp(0.1, 5.0);
        self.lod_distance_scale = self.lod_distance_scale.clamp(0.5, 2.0);
        self.shadow_max_distance = self.shadow_max_distance.clamp(4.0, 2048.0);
        self.shadow_softness = self.shadow_softness.clamp(0.0, 8.0);
        self.shadow_bias = self.shadow_bias.clamp(0.0, 0.1);
        self.shadow_normal_bias = self.shadow_normal_bias.clamp(0.0, 0.5);
        self.shadow_contact_strength = self.shadow_contact_strength.clamp(0.0, 1.0);
        self.shadow_pcss_light_radius_degrees =
            self.shadow_pcss_light_radius_degrees.clamp(0.001, 5.0);
        self.shadow_pcss_blocker_radius_texels =
            self.shadow_pcss_blocker_radius_texels.clamp(0.5, 32.0);
        self.shadow_pcss_max_filter_radius_texels =
            self.shadow_pcss_max_filter_radius_texels.clamp(0.5, 64.0);
        self.shadow_pcss_blocker_samples = self.shadow_pcss_blocker_samples.clamp(4, 16);
        self.shadow_pcss_filter_samples = self.shadow_pcss_filter_samples.clamp(4, 16);
        self.shadow_pcss_min_filter_radius_texels = self
            .shadow_pcss_min_filter_radius_texels
            .clamp(0.0, self.shadow_pcss_max_filter_radius_texels);
        self.shadow_pcss_stable_kernel_texels =
            self.shadow_pcss_stable_kernel_texels.clamp(1.0, 32.0);
        self.shadow_cascade_count = match self.shadow_cascade_count {
            0 => 0,
            value => value.clamp(1, 4),
        };
        self.shadow_map_resolution = normalize_shadow_map_resolution(self.shadow_map_resolution);
        if !self.shadows_enabled {
            self.shadow_quality = ShadowQuality::Off;
        } else if matches!(self.shadow_quality, ShadowQuality::Off) {
            self.shadow_quality = ShadowQuality::Balanced;
        }
    }

    pub fn apply_lod_quality(&mut self, quality: LodQuality) {
        self.lod_quality = quality;
        if let Some(scale) = quality.distance_scale() {
            self.lod_distance_scale = scale;
        }
        self.mark_custom();
    }

    #[inline]
    pub fn mark_custom(&mut self) {
        self.preset = GraphicsPreset::Custom;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupLaunchSettings {
    pub schema_version: u32,
    pub display: StartupDisplaySettings,
    pub graphics: StartupGraphicsSettings,
}

impl Default for StartupLaunchSettings {
    fn default() -> Self {
        Self {
            schema_version: STARTUP_SETTINGS_SCHEMA_VERSION,
            display: StartupDisplaySettings::default(),
            graphics: StartupGraphicsSettings::default(),
        }
    }
}

impl StartupLaunchSettings {
    pub fn normalize(&mut self) {
        self.schema_version = STARTUP_SETTINGS_SCHEMA_VERSION;
        self.display.monitor_index = self.display.monitor_index.max(-1);
        self.display.render_scale = self.display.render_scale.clamp(0.25, 2.0);
        self.display.refresh_rate_millihz = self.display.refresh_rate_millihz.min(1_000_000);
        self.display.frame_limit = match self.display.frame_limit {
            0 => 0,
            value => value.clamp(15, 1_000),
        };
        if !matches!(self.display.window_mode, StartupWindowMode::Windowed) {
            self.display.center_window = false;
        }
        self.graphics.normalize();
    }

    pub fn publish_process_variables(&self) {
        let mut value = self.clone();
        value.normalize();
        set_env(ENV_GRAPHICS_PRESET, value.graphics.preset.as_str());
        set_env(ENV_RENDER_SCALE, value.display.render_scale.to_string());
        set_env(ENV_MSAA_SAMPLES, value.graphics.msaa_samples.to_string());
        set_env(ENV_FXAA_ENABLED, bool_text(value.graphics.fxaa_enabled));
        set_env(
            ENV_FXAA_EDGE_THRESHOLD,
            value.graphics.fxaa_edge_threshold.to_string(),
        );
        set_env(
            ENV_FXAA_EDGE_THRESHOLD_MIN,
            value.graphics.fxaa_edge_threshold_min.to_string(),
        );
        set_env(
            ENV_FXAA_SUBPIXEL_QUALITY,
            value.graphics.fxaa_subpixel_quality.to_string(),
        );
        set_env(ENV_TAA_ENABLED, bool_text(value.graphics.taa_enabled));
        set_env(ENV_TAA_FEEDBACK, value.graphics.taa_feedback.to_string());
        set_env(
            ENV_TAA_NEIGHBORHOOD_CLAMPING,
            value.graphics.taa_neighborhood_clamping.to_string(),
        );
        set_env(
            ENV_TAA_JITTER_SCALE,
            value.graphics.taa_jitter_scale.to_string(),
        );
        set_env(ENV_SSAO_ENABLED, bool_text(value.graphics.ssao_enabled));
        set_env(
            ENV_SSAO_RADIUS_WS,
            value.graphics.ssao_radius_ws.to_string(),
        );
        set_env(
            ENV_SSAO_INTENSITY,
            value.graphics.ssao_intensity.to_string(),
        );
        set_env(
            ENV_SSAO_QUALITY_STEPS,
            value.graphics.ssao_quality_steps.to_string(),
        );
        set_env(
            ENV_SSAO_HALF_RESOLUTION,
            bool_text(value.graphics.ssao_half_resolution),
        );
        set_env(ENV_BLOOM_ENABLED, bool_text(value.graphics.bloom_enabled));
        set_env(
            ENV_BLOOM_THRESHOLD,
            value.graphics.bloom_threshold.to_string(),
        );
        set_env(ENV_BLOOM_KNEE, value.graphics.bloom_knee.to_string());
        set_env(
            ENV_BLOOM_INTENSITY,
            value.graphics.bloom_intensity.to_string(),
        );
        set_env(ENV_BLOOM_RADIUS, value.graphics.bloom_radius.to_string());
        set_env(
            ENV_DOF_ENABLED,
            bool_text(value.graphics.depth_of_field_enabled),
        );
        set_env(
            ENV_MOTION_BLUR_ENABLED,
            bool_text(value.graphics.motion_blur_enabled),
        );
        set_env(
            ENV_SUN_RAYS_ENABLED,
            bool_text(value.graphics.sun_rays_enabled),
        );
        set_env(
            ENV_SHADOWS_ENABLED,
            bool_text(value.graphics.shadows_enabled),
        );
        set_env(ENV_SHADOW_QUALITY, value.graphics.shadow_quality.as_str());
        set_env(
            ENV_SHADOW_CASCADE_COUNT,
            value.graphics.shadow_cascade_count.to_string(),
        );
        set_env(
            ENV_SHADOW_MAP_RESOLUTION,
            value.graphics.shadow_map_resolution.to_string(),
        );
        set_env(ENV_SHADOW_FILTER, value.graphics.shadow_filter.as_str());
        set_env(
            ENV_SHADOW_MAX_DISTANCE,
            value.graphics.shadow_max_distance.to_string(),
        );
        set_env(
            ENV_SHADOW_SOFTNESS,
            value.graphics.shadow_softness.to_string(),
        );
        set_env(ENV_SHADOW_BIAS, value.graphics.shadow_bias.to_string());
        set_env(
            ENV_SHADOW_NORMAL_BIAS,
            value.graphics.shadow_normal_bias.to_string(),
        );
        set_env(
            ENV_SHADOW_CONTACT_STRENGTH,
            value.graphics.shadow_contact_strength.to_string(),
        );
        set_env(
            ENV_SHADOW_PCSS_LIGHT_RADIUS_DEGREES,
            value.graphics.shadow_pcss_light_radius_degrees.to_string(),
        );
        set_env(
            ENV_SHADOW_PCSS_BLOCKER_RADIUS_TEXELS,
            value.graphics.shadow_pcss_blocker_radius_texels.to_string(),
        );
        set_env(
            ENV_SHADOW_PCSS_MAX_FILTER_RADIUS_TEXELS,
            value
                .graphics
                .shadow_pcss_max_filter_radius_texels
                .to_string(),
        );
        set_env(
            ENV_SHADOW_PCSS_BLOCKER_SAMPLES,
            value.graphics.shadow_pcss_blocker_samples.to_string(),
        );
        set_env(
            ENV_SHADOW_PCSS_FILTER_SAMPLES,
            value.graphics.shadow_pcss_filter_samples.to_string(),
        );
        set_env(
            ENV_SHADOW_PCSS_MIN_FILTER_RADIUS_TEXELS,
            value
                .graphics
                .shadow_pcss_min_filter_radius_texels
                .to_string(),
        );
        set_env(
            ENV_SHADOW_PCSS_STABLE_KERNEL_TEXELS,
            value.graphics.shadow_pcss_stable_kernel_texels.to_string(),
        );
        set_env(ENV_LOD_QUALITY, value.graphics.lod_quality.as_str());
        set_env(
            ENV_LOD_DISTANCE_SCALE,
            value.graphics.lod_distance_scale.to_string(),
        );
        set_env(ENV_TEXTURE_QUALITY, value.graphics.texture_quality.as_str());
        set_env(ENV_ANISOTROPY, value.graphics.anisotropy.to_string());
        set_env(ENV_WINDOW_MODE, value.display.window_mode.as_str());
        set_env(ENV_VSYNC, bool_text(value.display.vsync));
        set_env(
            ENV_REFRESH_RATE_MILLIHZ,
            value.display.refresh_rate_millihz.to_string(),
        );
        set_env(ENV_HDR_MODE, value.display.hdr.as_str());
        set_env(ENV_FRAME_LIMIT, value.display.frame_limit.to_string());
    }
}

static ACTIVE_SETTINGS: OnceLock<RwLock<StartupLaunchSettings>> = OnceLock::new();

pub fn startup_launch_settings() -> StartupLaunchSettings {
    ACTIVE_SETTINGS
        .get_or_init(|| RwLock::new(StartupLaunchSettings::default()))
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) fn set_startup_launch_settings(mut settings: StartupLaunchSettings) {
    settings.normalize();
    settings.publish_process_variables();
    let lock = ACTIVE_SETTINGS.get_or_init(|| RwLock::new(StartupLaunchSettings::default()));
    *lock
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
}

#[inline]
fn normalize_shadow_map_resolution(value: u32) -> u32 {
    if value == 0 {
        return 0;
    }
    match value {
        0..=256 => 256,
        257..=512 => 512,
        513..=1024 => 1024,
        1025..=2048 => 2048,
        2049..=4096 => 4096,
        4097..=8192 => 8192,
        _ => 16284,
    }
}

#[inline]
fn bool_text(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

#[inline]
fn set_env(key: &str, value: impl AsRef<str>) {
    std::env::set_var(key, value.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_independent_aa_and_expensive_effects() {
        let mut settings = StartupLaunchSettings::default();
        settings.graphics.preset = GraphicsPreset::Custom;
        settings.graphics.msaa_samples = 8;
        settings.graphics.fxaa_enabled = true;
        settings.graphics.taa_enabled = true;
        settings.graphics.ssao_enabled = true;
        settings.display.render_scale = 4.0;
        settings.normalize();

        assert_eq!(settings.graphics.msaa_samples, 8);
        assert!(settings.graphics.fxaa_enabled);
        assert!(settings.graphics.taa_enabled);
        assert!(settings.graphics.ssao_enabled);
        assert_eq!(settings.display.render_scale, 2.0);
    }

    #[test]
    fn disabled_shadows_force_off_quality() {
        let mut settings = StartupLaunchSettings::default();
        settings.graphics.shadows_enabled = false;
        settings.graphics.shadow_quality = ShadowQuality::Cinematic;
        settings.normalize();
        assert_eq!(settings.graphics.shadow_quality, ShadowQuality::Off);
    }

    #[test]
    fn normalizes_lod_and_shadow_overrides_without_forcing_scene_defaults() {
        let mut settings = StartupLaunchSettings::default();
        assert_eq!(settings.graphics.shadow_cascade_count, 0);
        assert_eq!(settings.graphics.shadow_map_resolution, 0);
        settings.graphics.lod_distance_scale = 9.0;
        settings.graphics.shadow_cascade_count = 99;
        settings.graphics.shadow_map_resolution = 3000;
        settings.normalize();
        assert_eq!(settings.graphics.lod_distance_scale, 2.0);
        assert_eq!(settings.graphics.shadow_cascade_count, 4);
        assert_eq!(settings.graphics.shadow_map_resolution, 4096);

        settings.graphics.shadow_map_resolution = 16284;
        settings.normalize();
        assert_eq!(settings.graphics.shadow_map_resolution, 16284);
    }

    #[test]
    fn preset_is_only_a_starting_point_and_controls_remain_independent() {
        let mut graphics = StartupGraphicsSettings::default();
        graphics.apply_preset(GraphicsPreset::High);
        graphics.fxaa_enabled = false;
        graphics.msaa_samples = 8;
        graphics.mark_custom();
        graphics.normalize();

        assert_eq!(graphics.preset, GraphicsPreset::Custom);
        assert_eq!(graphics.msaa_samples, 8);
        assert!(!graphics.fxaa_enabled);
        assert!(graphics.ssao_enabled);
    }
}
