use super::*;

#[derive(Debug)]
pub(super) struct AssetPreviewCameraState {
    yaw_pitch: AtomicU64,
    distance: AtomicU32,
    target_x: AtomicU32,
    target_y: AtomicU32,
    target_z: AtomicU32,
}

impl Default for AssetPreviewCameraState {
    fn default() -> Self {
        let view = AssetPreviewView::default();
        Self {
            yaw_pitch: AtomicU64::new(pack_f32_pair(view.yaw_radians, view.pitch_radians)),
            distance: AtomicU32::new(view.distance.to_bits()),
            target_x: AtomicU32::new(view.target_offset[0].to_bits()),
            target_y: AtomicU32::new(view.target_offset[1].to_bits()),
            target_z: AtomicU32::new(view.target_offset[2].to_bits()),
        }
    }
}

impl AssetPreviewCameraState {
    pub(super) fn snapshot(&self) -> AssetPreviewView {
        let (yaw_radians, pitch_radians) = unpack_f32_pair(self.yaw_pitch.load(Ordering::Acquire));
        AssetPreviewView {
            yaw_radians,
            pitch_radians,
            distance: f32::from_bits(self.distance.load(Ordering::Acquire)),
            target_offset: [
                f32::from_bits(self.target_x.load(Ordering::Acquire)),
                f32::from_bits(self.target_y.load(Ordering::Acquire)),
                f32::from_bits(self.target_z.load(Ordering::Acquire)),
            ],
        }
    }

    pub(super) fn reset(&self) -> AssetPreviewView {
        let view = AssetPreviewView::default();
        self.yaw_pitch.store(
            pack_f32_pair(view.yaw_radians, view.pitch_radians),
            Ordering::Release,
        );
        self.distance
            .store(view.distance.to_bits(), Ordering::Release);
        self.target_x
            .store(view.target_offset[0].to_bits(), Ordering::Release);
        self.target_y
            .store(view.target_offset[1].to_bits(), Ordering::Release);
        self.target_z
            .store(view.target_offset[2].to_bits(), Ordering::Release);
        view
    }

    pub(super) fn orbit(&self, dx_px: f32, dy_px: f32) -> AssetPreviewView {
        if !dx_px.is_finite() || !dy_px.is_finite() {
            return self.snapshot();
        }
        let _ = self
            .yaw_pitch
            .try_update(Ordering::AcqRel, Ordering::Acquire, |packed| {
                let (yaw, pitch) = unpack_f32_pair(packed);
                let next_yaw =
                    (yaw - dx_px * PREVIEW_ORBIT_SENSITIVITY).rem_euclid(std::f32::consts::TAU);
                let next_pitch = (pitch + dy_px * PREVIEW_ORBIT_SENSITIVITY)
                    .clamp(PREVIEW_MIN_PITCH, PREVIEW_MAX_PITCH);
                Some(pack_f32_pair(next_yaw, next_pitch))
            });
        self.snapshot()
    }

    pub(super) fn pan(&self, dx_px: f32, dy_px: f32) -> AssetPreviewView {
        if !dx_px.is_finite() || !dy_px.is_finite() {
            return self.snapshot();
        }
        if dx_px.abs() <= f32::EPSILON && dy_px.abs() <= f32::EPSILON {
            return self.snapshot();
        }

        let view = self.snapshot();
        let pitch = view
            .pitch_radians
            .clamp(PREVIEW_MIN_PITCH, PREVIEW_MAX_PITCH);
        let distance = view
            .distance
            .clamp(PREVIEW_MIN_DISTANCE, PREVIEW_MAX_DISTANCE);
        let horizontal = pitch.cos() * distance;
        let camera_offset = Vec3::new(
            view.yaw_radians.sin() * horizontal,
            pitch.sin() * distance,
            view.yaw_radians.cos() * horizontal,
        );
        let forward = (Vec3::ZERO - camera_offset).normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let world_units_per_pixel = distance * PREVIEW_PAN_SENSITIVITY;
        let delta = right * (-dx_px * world_units_per_pixel) + up * (dy_px * world_units_per_pixel);
        let current = Vec3::new(
            view.target_offset[0],
            view.target_offset[1],
            view.target_offset[2],
        );
        let mut next = current + delta;
        let length = next.length();
        if length > PREVIEW_MAX_TARGET_OFFSET {
            next = next.normalize_or_zero() * PREVIEW_MAX_TARGET_OFFSET;
        }
        self.target_x.store(next.x.to_bits(), Ordering::Release);
        self.target_y.store(next.y.to_bits(), Ordering::Release);
        self.target_z.store(next.z.to_bits(), Ordering::Release);
        self.snapshot()
    }

    pub(super) fn zoom(&self, wheel_y: f32) -> AssetPreviewView {
        if !wheel_y.is_finite() || wheel_y.abs() <= f32::EPSILON {
            return self.snapshot();
        }
        let steps = if wheel_y.abs() > 10.0 {
            (wheel_y / 120.0).clamp(-4.0, 4.0)
        } else {
            wheel_y.clamp(-4.0, 4.0)
        };
        let _ = self
            .distance
            .try_update(Ordering::AcqRel, Ordering::Acquire, |bits| {
                let current = f32::from_bits(bits);
                let next = (current * 0.86_f32.powf(steps))
                    .clamp(PREVIEW_MIN_DISTANCE, PREVIEW_MAX_DISTANCE);
                Some(next.to_bits())
            });
        self.snapshot()
    }
}

#[inline]
fn pack_f32_pair(a: f32, b: f32) -> u64 {
    (a.to_bits() as u64) | ((b.to_bits() as u64) << 32)
}

#[inline]
fn unpack_f32_pair(value: u64) -> (f32, f32) {
    (
        f32::from_bits(value as u32),
        f32::from_bits((value >> 32) as u32),
    )
}

impl AssetPreviewApi {
    pub fn camera_view(&self) -> AssetPreviewView {
        self.camera.snapshot()
    }

    pub fn orbit_camera(&self, dx_px: f32, dy_px: f32) -> Option<AssetPreviewView> {
        self.render_bundle().as_ref()?;
        let view = self.camera.orbit(dx_px, dy_px);
        self.viewport.request_external_redraw();
        Some(view)
    }

    pub fn pan_camera(&self, dx_px: f32, dy_px: f32) -> Option<AssetPreviewView> {
        self.render_bundle().as_ref()?;
        let view = self.camera.pan(dx_px, dy_px);
        self.viewport.request_external_redraw();
        Some(view)
    }

    pub fn zoom_camera(&self, wheel_y: f32) -> Option<AssetPreviewView> {
        self.render_bundle().as_ref()?;
        let view = self.camera.zoom(wheel_y);
        self.viewport.request_external_redraw();
        Some(view)
    }

    pub fn reset_camera(&self) -> Option<AssetPreviewView> {
        self.render_bundle().as_ref()?;
        let view = self.camera.reset();
        self.viewport.request_external_redraw();
        Some(view)
    }
}
