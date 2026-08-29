#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{CameraFrame, CameraFrameHistory, CameraViewport};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CameraViewportLayerId(pub u64);

impl Default for CameraViewportLayerId {
    #[inline]
    fn default() -> Self {
        Self(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraViewportFadeState {
    Idle,
    FadingIn,
    FadingOut,
}

impl Default for CameraViewportFadeState {
    #[inline]
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraViewportFade {
    pub state: CameraViewportFadeState,
    pub elapsed_sec: f32,
    pub duration_sec: f32,
    pub level: f32,
}

impl Default for CameraViewportFade {
    #[inline]
    fn default() -> Self {
        Self {
            state: CameraViewportFadeState::Idle,
            elapsed_sec: 0.0,
            duration_sec: 0.0,
            level: 0.0,
        }
    }
}

impl CameraViewportFade {
    #[inline]
    pub fn fade_in(duration_sec: f32) -> Self {
        Self {
            state: CameraViewportFadeState::FadingIn,
            elapsed_sec: 0.0,
            duration_sec: sanitize_duration(duration_sec),
            level: 1.0,
        }
    }

    #[inline]
    pub fn fade_out(duration_sec: f32) -> Self {
        Self {
            state: CameraViewportFadeState::FadingOut,
            elapsed_sec: 0.0,
            duration_sec: sanitize_duration(duration_sec),
            level: 0.0,
        }
    }

    #[inline]
    pub fn update(&mut self, dt: f32) {
        if matches!(self.state, CameraViewportFadeState::Idle) {
            return;
        }
        if dt.is_finite() && dt > 0.0 {
            self.elapsed_sec += dt;
        }
        let t = if self.duration_sec <= 0.0 {
            1.0
        } else {
            (self.elapsed_sec / self.duration_sec).clamp(0.0, 1.0)
        };
        match self.state {
            CameraViewportFadeState::Idle => {}
            CameraViewportFadeState::FadingIn => {
                self.level = 1.0 - t;
            }
            CameraViewportFadeState::FadingOut => {
                self.level = t;
            }
        }
        if t >= 1.0 {
            self.state = CameraViewportFadeState::Idle;
            self.elapsed_sec = 0.0;
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraViewportLayer {
    pub id: CameraViewportLayerId,
    pub viewport: CameraViewport,
    pub priority: i32,
    pub enabled: bool,
    pub fade: CameraViewportFade,
    pub last_frame: Option<CameraFrame>,
    pub history: CameraFrameHistory,
}

impl Default for CameraViewportLayer {
    #[inline]
    fn default() -> Self {
        Self {
            id: CameraViewportLayerId::default(),
            viewport: CameraViewport::default(),
            priority: 0,
            enabled: true,
            fade: CameraViewportFade::default(),
            last_frame: None,
            history: CameraFrameHistory::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraViewportManagerResource {
    pub layers: Vec<CameraViewportLayer>,
    active_layer: CameraViewportLayerId,
    last_active_viewport: Option<CameraViewport>,
    changed_this_update: bool,
}

impl Default for CameraViewportManagerResource {
    #[inline]
    fn default() -> Self {
        Self {
            layers: vec![CameraViewportLayer::default()],
            active_layer: CameraViewportLayerId::default(),
            last_active_viewport: None,
            changed_this_update: false,
        }
    }
}

impl CameraViewportManagerResource {
    #[inline]
    pub fn active_layer(&self) -> CameraViewportLayerId {
        self.active_layer
    }

    #[inline]
    pub fn changed_this_update(&self) -> bool {
        self.changed_this_update
    }

    #[inline]
    pub fn clear_changed_flag(&mut self) {
        self.changed_this_update = false;
    }

    pub fn set_layer(&mut self, layer: CameraViewportLayer) {
        if let Some(existing) = self.layers.iter_mut().find(|it| it.id == layer.id) {
            *existing = layer;
        } else {
            self.layers.push(layer);
        }
        self.layers.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        self.changed_this_update = true;
    }

    pub fn remove_layer(&mut self, id: CameraViewportLayerId) -> bool {
        let before = self.layers.len();
        self.layers.retain(|it| it.id != id);
        if self.layers.is_empty() {
            self.layers.push(CameraViewportLayer::default());
        }
        let removed = self.layers.len() != before;
        self.changed_this_update |= removed;
        removed
    }

    pub fn update(&mut self, dt: f32) {
        self.changed_this_update = false;
        for layer in &mut self.layers {
            layer.fade.update(dt);
        }
    }

    pub fn present_frame(&mut self, frame: CameraFrame, dt: f32) -> CameraFrame {
        let active_index = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.enabled)
            .max_by(|(_, a), (_, b)| {
                a.priority
                    .cmp(&b.priority)
                    .then_with(|| a.id.0.cmp(&b.id.0))
            })
            .map(|(index, _)| index)
            .unwrap_or(0);

        let active = &mut self.layers[active_index];
        let previous_id = self.active_layer;
        self.active_layer = active.id;
        let viewport = frame.viewport.sanitized();
        if self.last_active_viewport != Some(viewport) || previous_id != self.active_layer {
            self.changed_this_update = true;
        }
        active.viewport = viewport;
        active.last_frame = Some(frame);
        active.history.push(frame, dt);
        self.last_active_viewport = Some(viewport);
        frame
    }

    #[inline]
    pub fn active_viewport(&self) -> CameraViewport {
        self.last_active_viewport
            .unwrap_or_else(|| self.layers.last().map(|it| it.viewport).unwrap_or_default())
            .sanitized()
    }

    #[inline]
    pub fn active_history(&self) -> Option<CameraFrameHistory> {
        self.layers
            .iter()
            .find(|layer| layer.id == self.active_layer)
            .map(|layer| layer.history)
    }
}

#[inline]
fn sanitize_duration(duration_sec: f32) -> f32 {
    if duration_sec.is_finite() && duration_sec > 0.0 {
        duration_sec
    } else {
        0.0
    }
}
