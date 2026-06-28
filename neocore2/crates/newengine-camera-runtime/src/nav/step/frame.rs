use newengine_camera::{orbit_frame_sphere, CameraRig, RuntimeNavController, RuntimeNavMode};

use crate::nav::{BoundsSphere, CameraNavFrameRequest, CameraNavParams, CameraNavState};

#[inline]
pub(crate) fn maybe_frame_orbit(
    state: &mut CameraNavState,
    params: CameraNavParams,
    bounds: BoundsSphere,
    frame_req: CameraNavFrameRequest,
    explicit_frame: bool,
    user_busy: bool,
    ctrl: &mut RuntimeNavController,
    rig: &mut CameraRig,
) {
    debug_assert_eq!(ctrl.mode, RuntimeNavMode::Orbit);

    // Do not auto-reframe after the initial fit.
    //
    // Reframing on scene-bounds growth makes the entire world appear to jump whenever
    // a new actor is spawned or when an existing actor is moved outward. That also
    // invalidates runtime overlays perception because the camera changes under
    // the user during edit operations.
    //
    // Policy:
    // - frame once automatically on first usable scene
    // - frame again only on an explicit user request (F / Shift+F)
    let do_frame = explicit_frame || (!state.framed_once && !user_busy);

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
    orbit_frame_sphere(&mut ctrl.orbit, center, radius, fovy, params.aspect(), 1.15);

    state.framed_radius = radius;
    state.framed_once = true;

    ctrl.rebuild_orbit_rig(rig);
}
