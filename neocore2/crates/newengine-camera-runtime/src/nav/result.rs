use newengine_camera::{CameraFrame, RuntimeNavController};
use newengine_core::host_events::CursorState;

#[derive(Clone, Copy, Debug)]
pub struct CameraNavResult {
    pub frame: CameraFrame,
    pub controller: RuntimeNavController,
    pub cursor: CursorState,
}
