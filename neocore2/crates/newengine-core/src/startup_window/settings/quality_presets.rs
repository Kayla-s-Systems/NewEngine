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
