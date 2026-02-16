#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Quat, Vec2, Vec3};

use crate::modifiers::{AdsFov, HeadBob, NoiseShake, Recoil, SpringArm, Sway, TaaJitter, WeaponSway};
use crate::{CameraRig, CameraStack, Perspective, Projection};

/// A practical set of presets for gameplay cameras.
///
/// The presets only configure the stack. The engine is expected to supply
/// an anchor pose each frame through `CameraStackInput`.
pub struct GameCameraPresets;

impl GameCameraPresets {
    /// First-person camera stack.
    ///
    /// Anchor: character head or weapon camera socket.
    pub fn fps(viewport_w: u32, viewport_h: u32) -> CameraStack {
        let aspect = (viewport_w.max(1) as f32) / (viewport_h.max(1) as f32);

        let rig = CameraRig::new(Vec3::ZERO, Quat::IDENTITY);
        let proj = Projection::Perspective(Perspective::new(60.0_f32.to_radians(), aspect, 0.01, 10_000.0));

        let mut stack = CameraStack::new(rig, proj);
        stack.set_viewport(viewport_w, viewport_h);

        stack.push_modifier(Box::new(AdsFov::default()));
        stack.push_modifier(Box::new(WeaponSway::default()));
        stack.push_modifier(Box::new(Recoil::default()));
        stack.push_modifier(Box::new(HeadBob::default()));
        stack.push_modifier(Box::new(Sway::default()));
        stack.push_modifier(Box::new(NoiseShake::default()));
        stack.push_modifier(Box::new(TaaJitter::default()));

        stack
    }

    /// Third-person camera stack.
    ///
    /// Anchor: character pivot (feet) or chest. The default offsets assume a humanoid.
    pub fn tps(viewport_w: u32, viewport_h: u32) -> CameraStack {
        let aspect = (viewport_w.max(1) as f32) / (viewport_h.max(1) as f32);

        let rig = CameraRig::new(Vec3::ZERO, Quat::IDENTITY);
        let proj = Projection::Perspective(Perspective::new(60.0_f32.to_radians(), aspect, 0.01, 10_000.0));

        let mut stack = CameraStack::new(rig, proj);
        stack.set_viewport(viewport_w, viewport_h);

        let mut arm = SpringArm::default();
        arm.length = 3.2;
        arm.pos_smooth = 16.0;
        arm.rot_smooth = 16.0;
        arm.socket_offset_ls = Vec3::new(0.35, 1.65, 0.0);
        arm.look_at_offset_ls = Vec3::new(0.0, 1.45, 0.0);
        arm.collision_radius = 0.2;

        stack.push_modifier(Box::new(arm));
        stack.push_modifier(Box::new(AdsFov::new(60.0_f32.to_radians(), 45.0_f32.to_radians(), 14.0)));
        stack.push_modifier(Box::new(Sway::default()));
        stack.push_modifier(Box::new(NoiseShake::default()));
        let mut jitter = TaaJitter::default();
        jitter.scale_px = Vec2::new(0.5, 0.5);
        stack.push_modifier(Box::new(jitter));

        stack
    }
}
