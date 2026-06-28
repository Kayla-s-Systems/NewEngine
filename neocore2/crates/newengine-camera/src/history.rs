#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

use crate::CameraFrame;

/// Frame-to-frame camera history used by TAA, motion blur, streaming heuristics and diagnostics.
///
/// The camera domain owns this as plain data. Render backends should consume protocol snapshots or
/// renderer packets; they should not secretly reconstruct camera history from global mutable state.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraFrameHistory {
    previous: Option<CameraFrame>,
    current: Option<CameraFrame>,
    frame_index: u64,
    last_dt: f32,
    linear_velocity_ws: Vec3,
    angular_speed_rad: f32,
}

impl CameraFrameHistory {
    #[inline]
    pub const fn new() -> Self {
        Self {
            previous: None,
            current: None,
            frame_index: 0,
            last_dt: 0.0,
            linear_velocity_ws: Vec3::ZERO,
            angular_speed_rad: 0.0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    #[inline]
    pub fn push(&mut self, frame: CameraFrame, dt: f32) -> CameraHistorySample {
        let dt = sanitize_dt(dt);
        let previous = self.current;
        self.previous = previous;
        self.current = Some(frame);
        self.frame_index = self.frame_index.saturating_add(1);
        self.last_dt = dt;

        if let Some(prev) = previous {
            if dt > 0.0 {
                self.linear_velocity_ws = (frame.rig.position - prev.rig.position) / dt;
                self.angular_speed_rad = camera_angular_delta(prev, frame) / dt;
            } else {
                self.linear_velocity_ws = Vec3::ZERO;
                self.angular_speed_rad = 0.0;
            }
        } else {
            self.linear_velocity_ws = Vec3::ZERO;
            self.angular_speed_rad = 0.0;
        }

        CameraHistorySample {
            previous,
            current: frame,
            frame_index: self.frame_index,
            dt,
            linear_velocity_ws: self.linear_velocity_ws,
            angular_speed_rad: self.angular_speed_rad,
        }
    }

    #[inline]
    pub fn previous(&self) -> Option<CameraFrame> {
        self.previous
    }

    #[inline]
    pub fn current(&self) -> Option<CameraFrame> {
        self.current
    }

    #[inline]
    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    #[inline]
    pub fn last_dt(&self) -> f32 {
        self.last_dt
    }

    #[inline]
    pub fn linear_velocity_ws(&self) -> Vec3 {
        self.linear_velocity_ws
    }

    #[inline]
    pub fn angular_speed_rad(&self) -> f32 {
        self.angular_speed_rad
    }

    #[inline]
    pub fn has_previous(&self) -> bool {
        self.previous.is_some()
    }
}

/// Immutable result returned when a frame is pushed into `CameraFrameHistory`.
#[derive(Clone, Copy, Debug)]
pub struct CameraHistorySample {
    pub previous: Option<CameraFrame>,
    pub current: CameraFrame,
    pub frame_index: u64,
    pub dt: f32,
    pub linear_velocity_ws: Vec3,
    pub angular_speed_rad: f32,
}

impl CameraHistorySample {
    #[inline]
    pub fn has_previous(self) -> bool {
        self.previous.is_some()
    }
}

#[inline]
fn sanitize_dt(dt: f32) -> f32 {
    if dt.is_finite() && dt > 0.0 {
        dt
    } else {
        0.0
    }
}

#[inline]
fn camera_angular_delta(previous: CameraFrame, current: CameraFrame) -> f32 {
    let a = previous.rig.rotation.normalize_or_identity();
    let b = current.rig.rotation.normalize_or_identity();
    let dot = a.dot(b).abs().clamp(0.0, 1.0);
    // Unit quaternion dot encodes cos(theta/2).
    (2.0 * dot.acos()).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CameraChannel, CameraChannelState, CameraRig, CameraViewport, Perspective, Projection,
    };
    use newengine_math::{Quat, Vec2, Vec3};

    fn frame_at(position: Vec3) -> CameraFrame {
        CameraFrame::build(
            CameraChannelState::dominant(CameraChannel::Gameplay),
            CameraRig::new(position, Quat::IDENTITY),
            Projection::Perspective(Perspective::new(
                60.0f32.to_radians(),
                16.0 / 9.0,
                0.01,
                1000.0,
            )),
            CameraViewport::from_size(1280, 720),
            Vec2::ZERO,
        )
    }

    #[test]
    fn camera_history_reports_velocity_after_second_frame() {
        let mut history = CameraFrameHistory::new();
        let first = history.push(frame_at(Vec3::ZERO), 1.0 / 60.0);
        assert!(!first.has_previous());

        let second = history.push(frame_at(Vec3::new(10.0, 0.0, 0.0)), 0.5);
        assert!(second.has_previous());
        assert!((second.linear_velocity_ws.x - 20.0).abs() < 0.001);
        assert_eq!(history.frame_index(), 2);
    }
}
