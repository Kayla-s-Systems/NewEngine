use abi_stable::std_types::RString;
use abi_stable::StableAbi;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, StableAbi)]
pub struct PlatformLoadingOverlayV1 {
    pub active: bool,
    pub progress_01: f32,
    pub spinner_phase: u32,
    pub title: RString,
    pub status: RString,
    pub detail: RString,
    /// Structured system-layer overlay model serialized as JSON.
    pub view_json: RString,
}

impl Default for PlatformLoadingOverlayV1 {
    #[inline]
    fn default() -> Self {
        Self {
            active: false,
            progress_01: 0.0,
            spinner_phase: 0,
            title: RString::from(""),
            status: RString::from(""),
            detail: RString::from(""),
            view_json: RString::from(""),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, StableAbi)]
pub struct PlatformStepResultV1 {
    pub exit_requested: bool,
    pub loading_overlay: PlatformLoadingOverlayV1,
}

impl Default for PlatformStepResultV1 {
    #[inline]
    fn default() -> Self {
        Self {
            exit_requested: false,
            loading_overlay: PlatformLoadingOverlayV1::default(),
        }
    }
}
