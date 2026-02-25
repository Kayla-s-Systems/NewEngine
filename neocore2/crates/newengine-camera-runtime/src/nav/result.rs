use newengine_camera::{CameraRig, EditorNavController, Projection};
use newengine_core::host_events::CursorState;

#[derive(Clone, Copy, Debug)]
pub struct CameraNavResult {
    pub rig: CameraRig,
    pub controller: EditorNavController,
    pub projection: Projection,
    pub cursor: CursorState,
}