#[path = "dsp_types.rs"]
mod dsp_types;
use dsp_types::{CachedClip, EmbeddedYsncdClipLocator, YsncdRuntimeLayer, YsncdRuntimeMeta};

include!("dsp/spectral.rs");
include!("dsp/reverb_control.rs");
include!("dsp/direct_path.rs");
include!("dsp/reverb_tank.rs");

include!("dsp/spatial_environment.rs");

#[cfg(test)]
#[path = "dsp_tests.rs"]
mod dsp_tests;
