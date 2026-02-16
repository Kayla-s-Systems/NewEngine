#![forbid(unsafe_op_in_unsafe_fn)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiVisuals {
    Auto,
    Dark,
    Light,
}

impl Default for UiVisuals {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiDensity {
    Default,
    Compact,
    Dense,
    Tight,
}

impl Default for UiDensity {
    #[inline]
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone)]
pub struct UiThemeDesc {
    pub visuals: UiVisuals,
    pub scale: f32,
    pub font_size: f32,
    pub density: UiDensity,
}

impl Default for UiThemeDesc {
    #[inline]
    fn default() -> Self {
        Self {
            visuals: UiVisuals::Auto,
            scale: 1.0,
            font_size: 14.0,
            density: UiDensity::Default,
        }
    }
}