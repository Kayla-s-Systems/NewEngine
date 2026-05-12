use newengine_camera::{CameraFrame, EditorNavController};
use newengine_core::host_events::CursorState;

#[derive(Clone, Copy, Debug)]
pub struct CameraNavResult {
    pub frame: CameraFrame,
    pub controller: EditorNavController,
    pub cursor: CursorState,
}
