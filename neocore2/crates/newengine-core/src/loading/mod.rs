#![forbid(unsafe_op_in_unsafe_fn)]

mod boot_frame;
mod kernel;
mod profile;

#[cfg(test)]
mod tests;

pub use boot_frame::{
    BootDrawCommand, BootFrameDto, BootRect, BootTextRun, BootViewport, ColorRgba8,
    LoadingProgressSnapshot,
};
pub use kernel::EngineLoadingKernel;
pub use profile::{
    LoadingPhase, LoadingProfile, LoadingVisualRefs, LoadingVisualRole, ResolvedLoadingAssignment,
    ENGINE_LOADING_PLUGIN_ID,
};
