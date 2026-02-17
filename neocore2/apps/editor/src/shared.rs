#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(dead_code)]

use newengine_scene::Scene;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct EditorFlags {
    pub show_grid: bool,
    pub show_model: bool,
    pub auto_frame: bool,
}

impl Default for EditorFlags {
    fn default() -> Self {
        Self {
            show_grid: true,
            show_model: true,
            auto_frame: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct EditorRequests {
    pub load_model_path: Option<String>,
}

#[derive(Clone)]
pub struct EditorShared {
    pub scene: Arc<RwLock<Scene>>,
    pub flags: Arc<RwLock<EditorFlags>>,
    pub requests: Arc<Mutex<EditorRequests>>,
}

impl EditorShared {
    #[inline]
    pub fn new(scene: Scene) -> Self {
        Self {
            scene: Arc::new(RwLock::new(scene)),
            flags: Arc::new(RwLock::new(EditorFlags::default())),
            requests: Arc::new(Mutex::new(EditorRequests::default())),
        }
    }
}