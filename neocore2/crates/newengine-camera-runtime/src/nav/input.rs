use newengine_core::host_events::CursorState;
use newengine_math::{Vec2, Vec3};
use newengine_input_bindings::move_mask;

use newengine_camera::{CameraControlInput, RuntimeNavMode};

/// Minimal, renderer-agnostic viewport navigation input snapshot.
///
/// Produced by any UI/game layer and consumed by camera navigation systems.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraNavInput {
    pub dx_px: f32,
    pub dy_px: f32,
    pub wheel_y: f32,

    /// True when the viewport wants input (hovered, focused, or explicitly active).
    pub active: bool,

    /// True while look rotation is active.
    pub look_drag: bool,
    /// True while pan interaction is active.
    pub pan_drag: bool,
    /// True when UI is consuming input (gizmo, text input, etc.).
    pub ui_busy: bool,

    /// Latched free-fly intent (e.g. RMB capture).
    pub fly_rmb: bool,

    /// Semantic movement bitmask (`newengine-input-bindings::move_mask::*`).
    pub move_mask: u64,

    /// Additional user-controlled speed scalar from the runtime shell.
    pub speed_scalar: f32,
}

impl CameraNavInput {
    #[inline]
    pub fn clear_motion(&mut self) {
        self.dx_px = 0.0;
        self.dy_px = 0.0;
        self.wheel_y = 0.0;
    }
}

#[inline]
pub fn cursor_state_for_nav(input: &CameraNavInput) -> CursorState {
    if input.active && input.fly_rmb {
        CursorState::captured_locked()
    } else {
        CursorState::released()
    }
}

#[inline]
pub(crate) fn build_camera_input(input: &CameraNavInput, mode: RuntimeNavMode) -> CameraControlInput {
    let shift = (input.move_mask & move_mask::SPRINT) != 0;

    let fwd = ((input.move_mask & move_mask::FORWARD) != 0) as i32 - ((input.move_mask & move_mask::BACK) != 0) as i32;
    let right =
        ((input.move_mask & move_mask::RIGHT) != 0) as i32 - ((input.move_mask & move_mask::LEFT) != 0) as i32;
    let up = ((input.move_mask & move_mask::UP) != 0) as i32 - ((input.move_mask & move_mask::DOWN) != 0) as i32;

    let mut move_axis = Vec3::ZERO;
    let base_speed_mul = if shift { 2.0 } else { 1.0 };
    let shell_speed_mul = if input.speed_scalar.is_finite() && input.speed_scalar > 0.0 {
        input.speed_scalar
    } else {
        1.0
    };
    let speed_mul = base_speed_mul * shell_speed_mul;

    if input.pan_drag && mode == RuntimeNavMode::Orbit {
        move_axis.x = -input.dx_px;
        move_axis.y = input.dy_px;
    }

    if mode == RuntimeNavMode::Fly {
        move_axis.x = right as f32;
        move_axis.y = up as f32;
        move_axis.z = fwd as f32;
    }

    CameraControlInput {
        look_active: input.look_drag,
        look_delta: Vec2::new(-input.dx_px, -input.dy_px),
        move_axis,
        speed_mul,
        zoom_delta: input.wheel_y,
    }
}