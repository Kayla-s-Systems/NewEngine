#![forbid(unsafe_op_in_unsafe_fn)]

use crate::input_systems::InputActionFrameCarrier;
use newengine_input_actions_api::{CameraViewRequest, InputActionFrame, InputFrameSource};
use newengine_ui_api::UiInputFrame;

#[derive(Clone, Debug, Default)]
pub(super) struct ViewportInputSnap {
    pub dx_px: f32,
    pub dy_px: f32,
    pub wheel_y: f32,

    pub active: bool,

    pub look_drag: bool,
    pub pan_drag: bool,
    pub ui_busy: bool,
    pub fly_rmb: bool,

    /// Raw input listener/sampler was kept alive for this frame. UI capture must not turn this off.
    pub sampling_alive: bool,
    /// Policy gate: sampled camera deltas/actions are visible but must not drive navigation.
    pub camera_navigation_gated: bool,
    /// Policy gate: sampled movement actions are visible but must not drive gameplay locomotion.
    pub gameplay_movement_gated: bool,

    pub move_mask: u64,
    pub speed_scalar: f32,
    pub camera_view: CameraViewRequest,
    pub actions: InputActionFrame,
}

impl ViewportInputSnap {
    #[inline]
    pub(super) fn read(bridge: &crate::viewport_bridge::ViewportBridge) -> Self {
        let (
            dx_px,
            dy_px,
            wheel_y,
            active,
            look_drag,
            pan_drag,
            ui_busy,
            fly_rmb,
            move_mask,
            speed_scalar,
        ) = bridge.read_camera_input();
        Self {
            dx_px,
            dy_px,
            wheel_y,
            active,
            look_drag,
            pan_drag,
            ui_busy,
            fly_rmb,
            sampling_alive: true,
            camera_navigation_gated: false,
            gameplay_movement_gated: false,
            move_mask,
            speed_scalar,
            camera_view: CameraViewRequest::None,
            actions: InputActionFrame::default(),
        }
    }

    #[inline]
    pub(super) fn read_direct_surface(input: Option<&UiInputFrame>) -> Self {
        let Some(input) = input else {
            return Self::default();
        };
        let actions =
            newengine_input_bindings_runtime::resolve_input_actions(&UiInputSource(input));
        let middle_drag = input.is_mouse_down(newengine_input_api::mouse_button::MIDDLE);

        Self {
            dx_px: input.mouse_delta.0 + actions.look_axis[0] * 18.0,
            dy_px: input.mouse_delta.1 + actions.look_axis[1] * 18.0,
            wheel_y: input.mouse_wheel.1,
            active: true,
            look_drag: !middle_drag,
            pan_drag: middle_drag,
            ui_busy: false,
            fly_rmb: false,
            sampling_alive: true,
            camera_navigation_gated: false,
            gameplay_movement_gated: false,
            move_mask: actions.move_mask,
            speed_scalar: 1.0,
            camera_view: actions.camera_view,
            actions,
        }
    }

    /// Merge semantic engine.input.bindings actions from the host/UI input frame into
    /// a viewport-bridge snapshot.
    ///
    /// The viewport bridge owns camera mouse deltas for the normal playable surface,
    /// while the InputPlugin/UiInputFrame owns keyboard/gamepad semantic actions such
    /// as primary UI toggle, F1 editor tools and UI navigation. Those two streams must be
    /// composed every frame; otherwise gameplay can run, but modal UI actions only work
    /// in direct-surface debug paths.
    #[inline]
    pub(super) fn merge_semantic_actions_from_surface(
        &mut self,
        input: Option<&UiInputFrame>,
        canonical_mouse_authoritative: bool,
    ) {
        let Some(input) = input else {
            return;
        };
        let actions =
            newengine_input_bindings_runtime::resolve_input_actions(&UiInputSource(input));
        let middle_drag = input.is_mouse_down(newengine_input_api::mouse_button::MIDDLE);
        self.pan_drag = middle_drag;
        if middle_drag {
            // MMB owns the mouse delta for camera dolly in gameplay Orbit. Do not let the
            // same packet simultaneously rotate the camera.
            self.look_drag = false;
        }

        // Canonical engine.input owns raw mouse deltas for both direct and normal
        // playable surfaces. ViewportBridge is a legacy/editor fallback. Adding both
        // packets double-counted identical events and, with opposite Y conventions,
        // could cancel pitch while yaw still worked. Canonical mouse motion therefore
        // replaces the legacy packet whenever it is present; gamepad look is additive.
        let canonical_dx = if input.mouse_delta.0.is_finite() {
            input.mouse_delta.0
        } else {
            0.0
        };
        let canonical_dy = if input.mouse_delta.1.is_finite() {
            input.mouse_delta.1
        } else {
            0.0
        };
        let raw_mouse_look = canonical_dx.abs() > f32::EPSILON || canonical_dy.abs() > f32::EPSILON;
        if canonical_mouse_authoritative || raw_mouse_look {
            self.dx_px = canonical_dx;
            self.dy_px = canonical_dy;
        }
        self.dx_px += actions.look_axis[0] * 18.0;
        self.dy_px += actions.look_axis[1] * 18.0;
        self.wheel_y += input.mouse_wheel.1;
        self.move_mask |= actions.move_mask;
        if raw_mouse_look || actions.look_axis != [0.0, 0.0] {
            self.active = true;
            if !middle_drag {
                self.look_drag = true;
            }
        }
        if !matches!(actions.camera_view, CameraViewRequest::None) {
            self.camera_view = actions.camera_view;
        }
        self.actions = actions;
    }

    #[inline]
    pub(super) fn apply_gameplay_input_handoff(
        &mut self,
        policy: &super::super::runtime_profile::GameplayInputProfile,
    ) {
        if policy.force_gameplay_actions {
            self.move_mask |= self.actions.move_mask;
        }
        if policy.force_gameplay_look {
            self.active = true;
            if !self.pan_drag {
                self.look_drag = true;
            }
            self.ui_busy = false;
            self.fly_rmb = policy.capture_cursor_on_play;
        }
    }

    /// Reclaims only the viewport-navigation channel for unified Editor Mode.
    /// Gameplay actions stay capture-gated, while RMB+WASD/QE drives the generic
    /// camera-runtime Fly controller. Fly has no physics body, so this is also the
    /// editor's noclip implementation rather than a gameplay cheat toggle.
    pub(super) fn apply_editor_fly_navigation(
        &mut self,
        input: Option<&UiInputFrame>,
        pointer_in_viewport: bool,
        camera_allowed: bool,
    ) {
        let Some(input) = input else {
            self.fly_rmb = false;
            self.look_drag = false;
            self.move_mask = 0;
            return;
        };
        if !pointer_in_viewport || !camera_allowed {
            self.fly_rmb = false;
            self.look_drag = false;
            self.pan_drag = false;
            self.move_mask = 0;
            return;
        }

        let actions =
            newengine_input_bindings_runtime::resolve_input_actions(&UiInputSource(input));
        let rmb = input.is_mouse_down(newengine_input_api::mouse_button::RIGHT);
        self.active = true;
        self.camera_navigation_gated = false;
        self.gameplay_movement_gated = true;
        self.fly_rmb = rmb;
        self.look_drag = rmb;
        self.pan_drag = false;
        self.ui_busy = !rmb;
        self.camera_view = CameraViewRequest::None;
        self.speed_scalar = 1.0;
        self.move_mask = if rmb { actions.move_mask } else { 0 };
        if rmb {
            let canonical_dx = if input.mouse_delta.0.is_finite() {
                input.mouse_delta.0
            } else {
                0.0
            };
            let canonical_dy = if input.mouse_delta.1.is_finite() {
                input.mouse_delta.1
            } else {
                0.0
            };
            self.dx_px = canonical_dx + actions.look_axis[0] * 18.0;
            self.dy_px = canonical_dy + actions.look_axis[1] * 18.0;
            self.wheel_y = input.mouse_wheel.1;
        } else {
            self.dx_px = 0.0;
            self.dy_px = 0.0;
            self.wheel_y = 0.0;
        }
    }

    #[inline]
    pub(super) fn action_carrier(&mut self) -> InputActionFrameCarrier<'_> {
        InputActionFrameCarrier {
            dx_px: &mut self.dx_px,
            dy_px: &mut self.dy_px,
            wheel_y: &mut self.wheel_y,
            active: &mut self.active,
            look_drag: &mut self.look_drag,
            pan_drag: &mut self.pan_drag,
            ui_busy: &mut self.ui_busy,
            fly_rmb: &mut self.fly_rmb,
            sampling_alive: &mut self.sampling_alive,
            camera_navigation_gated: &mut self.camera_navigation_gated,
            gameplay_movement_gated: &mut self.gameplay_movement_gated,
            move_mask: &mut self.move_mask,
            speed_scalar: &mut self.speed_scalar,
            camera_view: &mut self.camera_view,
            actions: &mut self.actions,
        }
    }
}

struct UiInputSource<'a>(&'a UiInputFrame);

impl InputFrameSource for UiInputSource<'_> {
    #[inline]
    fn is_key_down(&self, key: u32) -> bool {
        self.0.is_key_down(key)
    }
    #[inline]
    fn is_key_pressed(&self, key: u32) -> bool {
        self.0.is_key_pressed(key)
    }
    #[inline]
    fn is_key_released(&self, key: u32) -> bool {
        self.0.keys_released.contains(&key)
    }
    #[inline]
    fn is_mouse_down(&self, button: u32) -> bool {
        self.0.is_mouse_down(button)
    }
    #[inline]
    fn is_mouse_pressed(&self, button: u32) -> bool {
        self.0.is_mouse_pressed(button)
    }
    #[inline]
    fn is_mouse_released(&self, button: u32) -> bool {
        self.0.mouse_released.contains(&button)
    }
    #[inline]
    fn has_gamepad_connected(&self) -> bool {
        self.0.has_gamepad_connected()
    }

    #[inline]
    fn is_gamepad_button_down(&self, button: &str) -> bool {
        self.0.is_gamepad_button_down(button)
    }
    #[inline]
    fn is_gamepad_button_pressed(&self, button: &str) -> bool {
        self.0.is_gamepad_button_pressed(button)
    }
    #[inline]
    fn is_gamepad_button_released(&self, button: &str) -> bool {
        self.0.is_gamepad_button_released(button)
    }
    #[inline]
    fn gamepad_axis(&self, axis: &str) -> f32 {
        self.0.gamepad_axis(axis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_playable_surface_merges_canonical_mouse_xy() {
        let mut snap = ViewportInputSnap::default();
        let mut frame = UiInputFrame::default();
        frame.mouse_delta = (7.25, -5.5);

        snap.merge_semantic_actions_from_surface(Some(&frame), true);

        assert!((snap.dx_px - 7.25).abs() <= f32::EPSILON);
        assert!((snap.dy_px + 5.5).abs() <= f32::EPSILON);
        assert!(snap.active, "raw mouse motion must activate gameplay look");
        assert!(
            snap.look_drag,
            "raw mouse motion must own look even without semantic stick input"
        );
    }

    #[test]
    fn pure_vertical_raw_mouse_delta_activates_look_without_x_motion() {
        let mut snap = ViewportInputSnap::default();
        let mut frame = UiInputFrame::default();
        frame.mouse_delta = (0.0, -8.0);

        snap.merge_semantic_actions_from_surface(Some(&frame), true);

        assert!(snap.dx_px.abs() <= f32::EPSILON);
        assert!((snap.dy_px + 8.0).abs() <= f32::EPSILON);
        assert!(snap.active);
        assert!(snap.look_drag);
    }

    #[test]
    fn canonical_mouse_delta_overrides_duplicate_legacy_viewport_packet() {
        let mut snap = ViewportInputSnap {
            dx_px: 2.0,
            dy_px: 3.0,
            ..ViewportInputSnap::default()
        };
        let mut frame = UiInputFrame::default();
        frame.mouse_delta = (4.0, -8.0);

        snap.merge_semantic_actions_from_surface(Some(&frame), true);

        assert!((snap.dx_px - 4.0).abs() <= f32::EPSILON);
        assert!((snap.dy_px + 8.0).abs() <= f32::EPSILON);
    }
    #[test]
    fn gameplay_canonical_zero_clears_stale_legacy_mouse_packet() {
        let mut snap = ViewportInputSnap {
            dx_px: 42.0,
            dy_px: -35.0,
            ..ViewportInputSnap::default()
        };
        let frame = UiInputFrame::default();

        snap.merge_semantic_actions_from_surface(Some(&frame), true);

        assert_eq!(snap.dx_px, 0.0);
        assert_eq!(snap.dy_px, 0.0);
    }

    #[test]
    fn editor_zero_canonical_mouse_preserves_legacy_viewport_packet() {
        let mut snap = ViewportInputSnap {
            dx_px: 7.0,
            dy_px: -3.0,
            ..ViewportInputSnap::default()
        };
        let frame = UiInputFrame::default();

        snap.merge_semantic_actions_from_surface(Some(&frame), false);

        assert_eq!(snap.dx_px, 7.0);
        assert_eq!(snap.dy_px, -3.0);
    }

    #[test]
    fn middle_mouse_drag_is_preserved_as_dolly_channel() {
        let mut snap = ViewportInputSnap::default();
        let mut frame = UiInputFrame::default();
        frame.mouse_delta = (4.0, -12.0);
        frame
            .mouse_down
            .insert(newengine_input_api::mouse_button::MIDDLE);

        snap.merge_semantic_actions_from_surface(Some(&frame), true);

        assert!(snap.pan_drag);
        assert!(!snap.look_drag);
        assert_eq!(snap.dx_px, 4.0);
        assert_eq!(snap.dy_px, -12.0);
    }

    #[test]
    fn editor_fly_reclaims_camera_motion_but_keeps_gameplay_gated() {
        let mut snap = ViewportInputSnap {
            camera_navigation_gated: true,
            gameplay_movement_gated: true,
            ..ViewportInputSnap::default()
        };
        let mut frame = UiInputFrame::default();
        frame.mouse_delta = (6.0, -4.0);
        frame
            .mouse_down
            .insert(newengine_input_api::mouse_button::RIGHT);

        snap.apply_editor_fly_navigation(Some(&frame), true, true);

        assert!(snap.fly_rmb);
        assert!(snap.look_drag);
        assert!(!snap.camera_navigation_gated);
        assert!(snap.gameplay_movement_gated);
        assert_eq!(snap.dx_px, 6.0);
        assert_eq!(snap.dy_px, -4.0);
    }

    #[test]
    fn editor_fly_does_not_capture_outside_viewport() {
        let mut snap = ViewportInputSnap::default();
        let mut frame = UiInputFrame::default();
        frame
            .mouse_down
            .insert(newengine_input_api::mouse_button::RIGHT);
        snap.apply_editor_fly_navigation(Some(&frame), false, true);
        assert!(!snap.fly_rmb);
        assert!(!snap.look_drag);
        assert_eq!(snap.move_mask, 0);
    }
}
