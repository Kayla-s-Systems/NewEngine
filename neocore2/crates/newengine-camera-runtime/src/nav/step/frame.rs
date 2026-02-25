use newengine_camera::{orbit_frame_sphere, CameraRig, EditorNavController, EditorNavMode};

use crate::nav::{BoundsSphere, CameraNavFrameRequest, CameraNavParams, CameraNavState};

#[inline]
pub(crate) fn maybe_frame_orbit(
    state: &mut CameraNavState,
    params: CameraNavParams,
    bounds: BoundsSphere,
    frame_req: CameraNavFrameRequest,
    explicit_frame: bool,
    user_busy: bool,
    ctrl: &mut EditorNavController,
    rig: &mut CameraRig,
) {
    debug_assert_eq!(ctrl.mode, EditorNavMode::Orbit);

    let grew = bounds.radius > state.framed_radius * 1.05;
    let do_frame = explicit_frame
        || (!state.framed_once && !user_busy)
        || (state.framed_once && !user_busy && grew);

    if !do_frame {
        return;
    }

    let (center, radius) = if explicit_frame && !frame_req.all {
        if let Some(sb) = params.selection_bounds {
            (sb.center, sb.radius)
        } else {
            (bounds.center, bounds.radius)
        }
    } else {
        (bounds.center, bounds.radius)
    };

    let fovy = 60.0f32.to_radians();
    orbit_frame_sphere(&mut ctrl.orbit, center, radius, fovy, params.aspect, 1.15);

    state.framed_radius = radius;
    state.framed_once = true;

    ctrl.rebuild_orbit_rig(rig);
}