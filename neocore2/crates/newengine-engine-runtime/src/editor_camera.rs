#![forbid(unsafe_op_in_unsafe_fn)]

// Runtime camera is a pure adapter: all navigation logic lives in `newengine-camera`.

pub use newengine_camera::EditorNavController as RuntimeCameraController;
