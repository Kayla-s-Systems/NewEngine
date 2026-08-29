#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeSet;

use newengine_input_actions_api::{move_mask, CameraViewRequest, InputActionFrame};

/// Mutable carrier used by the render controller so input policy does not own
/// viewport snapshots directly.
pub struct InputActionFrameCarrier<'a> {
    pub dx_px: &'a mut f32,
    pub dy_px: &'a mut f32,
    pub wheel_y: &'a mut f32,
    pub active: &'a mut bool,
    pub look_drag: &'a mut bool,
    pub pan_drag: &'a mut bool,
    pub ui_busy: &'a mut bool,
    pub fly_rmb: &'a mut bool,
    /// Listener/sampler liveness invariant: UI may gate navigation, never kill sampling.
    pub sampling_alive: &'a mut bool,
    /// Per-frame policy gate for camera look/navigation application.
    pub camera_navigation_gated: &'a mut bool,
    /// Per-frame policy gate for gameplay movement application.
    pub gameplay_movement_gated: &'a mut bool,
    pub move_mask: &'a mut u64,
    pub speed_scalar: &'a mut f32,
    pub camera_view: &'a mut CameraViewRequest,
    pub actions: &'a mut InputActionFrame,
}

impl InputActionFrameCarrier<'_> {
    pub(super) fn suppress_all(&mut self) {
        self.suppress_runtime_controls();
        self.suppress_actions();
    }

    pub(super) fn suppress_actions(&mut self) {
        *self.move_mask = 0;
        *self.camera_view = CameraViewRequest::None;
        *self.actions = InputActionFrame::default();
    }

    pub(super) fn suppress_gameplay_actions(&mut self) {
        const GAMEPLAY_LISTENER: &str = "newengine-gameplay:player-controller";
        let blocked = self
            .actions
            .events
            .iter()
            .filter(|event| {
                event
                    .listeners
                    .iter()
                    .any(|listener| listener == GAMEPLAY_LISTENER)
            })
            .map(|event| event.action.clone())
            .collect::<BTreeSet<_>>();
        if blocked.is_empty() {
            return;
        }
        self.actions
            .actions
            .retain(|action| !blocked.contains(action));
        self.actions
            .events
            .retain(|event| !blocked.contains(&event.action));
    }

    pub(super) fn suppress_camera_look(&mut self) {
        *self.dx_px = 0.0;
        *self.dy_px = 0.0;
        self.actions.look_axis = [0.0, 0.0];
    }

    pub(super) fn suppress_gameplay_movement(&mut self) {
        self.actions.move_mask &= !(move_mask::FORWARD
            | move_mask::BACK
            | move_mask::LEFT
            | move_mask::RIGHT
            | move_mask::UP
            | move_mask::DOWN
            | move_mask::SPRINT);
        self.actions.move_axis = [0.0, 0.0, 0.0];
        self.actions.sprint = false;
        *self.move_mask = 0;
        *self.speed_scalar = 1.0;
    }

    pub(super) fn suppress_ui_navigation(&mut self) {
        self.actions.ui_toggle = false;
        self.actions.ui_accept = false;
        self.actions.ui_back = false;
        self.actions.ui_nav = [0, 0];
    }

    pub(super) fn suppress_gamepad_effects(&mut self) {
        self.actions.look_axis = [0.0, 0.0];
        self.suppress_gameplay_movement();
    }

    pub(super) fn suppress_runtime_controls(&mut self) {
        self.gate_runtime_navigation_by_ui();
        *self.active = false;
        *self.look_drag = false;
        *self.pan_drag = false;
    }

    /// Applies profile-owned gameplay capture without collapsing it into an engine-wide modal.
    ///
    /// This keeps low-level sampling alive while allowing providers to independently gate
    /// semantic gameplay actions, camera navigation and player locomotion.
    pub fn apply_gameplay_input_capture(
        &mut self,
        capture: newengine_input_capture_api::GameplayInputCapture,
    ) {
        if capture.pointer || capture.keyboard {
            *self.ui_busy = true;
        }
        if capture.block_gameplay_actions {
            self.suppress_gameplay_actions();
        }
        if capture.block_camera_navigation {
            *self.sampling_alive = true;
            *self.camera_navigation_gated = true;
            *self.dx_px = 0.0;
            *self.dy_px = 0.0;
            *self.wheel_y = 0.0;
            *self.active = false;
            *self.look_drag = false;
            *self.pan_drag = false;
            *self.fly_rmb = false;
            *self.camera_view = CameraViewRequest::None;
            self.actions.look_axis = [0.0, 0.0];
            self.actions.camera_view = CameraViewRequest::None;
        }
        if capture.block_player_movement {
            *self.gameplay_movement_gated = true;
            self.suppress_gameplay_movement();
        }
    }

    /// Gate camera navigation while keeping raw sampling/listeners alive.
    pub(super) fn gate_camera_navigation_by_ui(&mut self) {
        *self.sampling_alive = true;
        *self.camera_navigation_gated = true;
        *self.dx_px = 0.0;
        *self.dy_px = 0.0;
        *self.wheel_y = 0.0;
        *self.ui_busy = true;
        *self.fly_rmb = false;
        *self.camera_view = CameraViewRequest::None;
        self.actions.look_axis = [0.0, 0.0];
        self.actions.camera_view = CameraViewRequest::None;
    }

    /// Gate player locomotion without consuming camera look.
    pub(super) fn gate_gameplay_movement_by_ui(&mut self) {
        *self.sampling_alive = true;
        *self.gameplay_movement_gated = true;
        *self.ui_busy = true;
        self.suppress_gameplay_movement();
    }

    /// Full modal gate: both navigation channels are blocked, while UI actions survive.
    pub(super) fn gate_runtime_navigation_by_ui(&mut self) {
        self.gate_camera_navigation_by_ui();
        self.gate_gameplay_movement_by_ui();
        self.suppress_gameplay_actions();
    }
}

#[inline]
pub(super) fn action_frame_has_activity(frame: &InputActionFrame) -> bool {
    frame.move_mask != 0
        || frame.move_axis != [0.0, 0.0, 0.0]
        || frame.look_axis != [0.0, 0.0]
        || frame.sprint
        || !matches!(frame.camera_view, CameraViewRequest::None)
        || frame.ui_toggle
        || frame.ui_accept
        || frame.ui_back
        || frame.ui_nav != [0, 0]
        || !frame.actions.is_empty()
}

#[inline]
pub(super) fn movement_has_activity(frame: &InputActionFrame) -> bool {
    frame.move_mask
        & (move_mask::FORWARD
            | move_mask::BACK
            | move_mask::LEFT
            | move_mask::RIGHT
            | move_mask::UP
            | move_mask::DOWN
            | move_mask::SPRINT)
        != 0
        || frame.move_axis != [0.0, 0.0, 0.0]
        || frame.sprint
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_input_actions_api::InputActionDispatchEvent;

    #[test]
    fn modal_gate_preserves_non_gameplay_controller_actions() {
        let mut dx_px = 0.0;
        let mut dy_px = 0.0;
        let mut wheel_y = 0.0;
        let mut active = true;
        let mut look_drag = false;
        let mut pan_drag = false;
        let mut ui_busy = false;
        let mut fly_rmb = false;
        let mut sampling_alive = false;
        let mut camera_navigation_gated = false;
        let mut gameplay_movement_gated = false;
        let mut move_mask = 0;
        let mut speed_scalar = 1.0;
        let mut camera_view = CameraViewRequest::None;
        let mut actions = InputActionFrame {
            actions: vec!["player.fire.primary".into(), "ui.modal.toggle".into()],
            events: vec![
                InputActionDispatchEvent {
                    action: "player.fire.primary".into(),
                    listeners: vec!["newengine-gameplay:player-controller".into()],
                    consumed_by: None,
                },
                InputActionDispatchEvent {
                    action: "ui.modal.toggle".into(),
                    listeners: vec!["gameplay-ui:modal-controller".into()],
                    consumed_by: None,
                },
            ],
            ..InputActionFrame::default()
        };
        let mut carrier = InputActionFrameCarrier {
            dx_px: &mut dx_px,
            dy_px: &mut dy_px,
            wheel_y: &mut wheel_y,
            active: &mut active,
            look_drag: &mut look_drag,
            pan_drag: &mut pan_drag,
            ui_busy: &mut ui_busy,
            fly_rmb: &mut fly_rmb,
            sampling_alive: &mut sampling_alive,
            camera_navigation_gated: &mut camera_navigation_gated,
            gameplay_movement_gated: &mut gameplay_movement_gated,
            move_mask: &mut move_mask,
            speed_scalar: &mut speed_scalar,
            camera_view: &mut camera_view,
            actions: &mut actions,
        };
        carrier.gate_runtime_navigation_by_ui();
        assert_eq!(carrier.actions.actions, ["ui.modal.toggle"]);
        assert_eq!(carrier.actions.events.len(), 1);
    }

    #[test]
    fn modal_gate_removes_gameplay_actions_but_preserves_ui_actions() {
        let mut dx_px = 4.0;
        let mut dy_px = 3.0;
        let mut wheel_y = 1.0;
        let mut active = true;
        let mut look_drag = true;
        let mut pan_drag = true;
        let mut ui_busy = false;
        let mut fly_rmb = true;
        let mut sampling_alive = false;
        let mut camera_navigation_gated = false;
        let mut gameplay_movement_gated = false;
        let mut move_mask = move_mask::FORWARD;
        let mut speed_scalar = 2.0;
        let mut camera_view = CameraViewRequest::Next;
        let mut actions = InputActionFrame {
            actions: vec!["player.fire.primary".into(), "ui.accept".into()],
            events: vec![
                InputActionDispatchEvent {
                    action: "player.fire.primary".into(),
                    listeners: vec!["newengine-gameplay:player-controller".into()],
                    consumed_by: None,
                },
                InputActionDispatchEvent {
                    action: "ui.accept".into(),
                    listeners: vec!["newengine-ui:ui-navigation".into()],
                    consumed_by: Some("newengine-ui:ui-navigation".into()),
                },
            ],
            ..InputActionFrame::default()
        };
        let mut carrier = InputActionFrameCarrier {
            dx_px: &mut dx_px,
            dy_px: &mut dy_px,
            wheel_y: &mut wheel_y,
            active: &mut active,
            look_drag: &mut look_drag,
            pan_drag: &mut pan_drag,
            ui_busy: &mut ui_busy,
            fly_rmb: &mut fly_rmb,
            sampling_alive: &mut sampling_alive,
            camera_navigation_gated: &mut camera_navigation_gated,
            gameplay_movement_gated: &mut gameplay_movement_gated,
            move_mask: &mut move_mask,
            speed_scalar: &mut speed_scalar,
            camera_view: &mut camera_view,
            actions: &mut actions,
        };

        carrier.gate_runtime_navigation_by_ui();

        assert_eq!(carrier.actions.actions, ["ui.accept"]);
        assert_eq!(carrier.actions.events.len(), 1);
        assert_eq!(carrier.actions.events[0].action, "ui.accept");
        assert!(*carrier.sampling_alive);
        assert!(*carrier.camera_navigation_gated);
        assert!(*carrier.gameplay_movement_gated);
    }
}
