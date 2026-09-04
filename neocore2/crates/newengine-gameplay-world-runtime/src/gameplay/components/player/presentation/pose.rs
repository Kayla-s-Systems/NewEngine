/// Two authoritative fixed-step poses retained for render interpolation.
/// Simulation/physics continue to own `Transform`; this component is presentation history only.
#[derive(Clone, Copy, Debug)]
pub struct PlayerFixedPoseHistory {
    pub previous_position: Vec3,
    pub previous_rotation: newengine_math::Quat,
    pub current_position: Vec3,
    pub current_rotation: newengine_math::Quat,
    pub current_fixed_tick: u64,
    pub initialized: bool,
}

impl Default for PlayerFixedPoseHistory {
    fn default() -> Self {
        Self {
            previous_position: Vec3::ZERO,
            previous_rotation: newengine_math::Quat::IDENTITY,
            current_position: Vec3::ZERO,
            current_rotation: newengine_math::Quat::IDENTITY,
            current_fixed_tick: 0,
            initialized: false,
        }
    }
}

/// Render-cadence player pose sampled between the two latest fixed simulation poses.
/// Camera and player visuals consume the same value so third-person framing cannot jitter.
#[derive(Clone, Copy, Debug)]
pub struct PlayerRenderPose {
    pub position: Vec3,
    pub rotation: newengine_math::Quat,
    pub simulation_position: Vec3,
    pub simulation_rotation: newengine_math::Quat,
    pub fixed_alpha: f32,
    pub source_fixed_tick: u64,
}

impl Default for PlayerRenderPose {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: newengine_math::Quat::IDENTITY,
            simulation_position: Vec3::ZERO,
            simulation_rotation: newengine_math::Quat::IDENTITY,
            fixed_alpha: 0.0,
            source_fixed_tick: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerModelBinding {
    pub assignment_revision: u64,
    pub source: String,
    pub skeleton_source: Option<String>,
    pub visual_root: Option<newengine_ecs::EntityId>,
    pub part_count: u32,
    pub target_height: f32,
    pub feet_to_eye_height: f32,
}

impl Default for PlayerModelBinding {
    #[inline]
    fn default() -> Self {
        Self {
            assignment_revision: 0,
            source: String::new(),
            skeleton_source: None,
            visual_root: None,
            part_count: 0,
            target_height: 1.80,
            feet_to_eye_height: 1.64,
        }
    }
}

/// Provider-neutral first-person camera anchor published by the active avatar/runtime model.
/// The position is a stable render-cadence eye center in world space. Camera orientation remains
/// input-owned (CharacterMotor yaw/pitch); animation may affect presentation but never owns the
/// gameplay camera position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerFirstPersonCameraAnchor {
    pub eye_center_ws: Vec3,
    /// Optional exceptional ADS camera anchor (for authored optics/cinematics). Ordinary no-scope
    /// ironsight keeps this `None`: the stable camera is authoritative and weapon/arms move to its
    /// sight frame. Camera runtime only performs temporal blending; orientation remains input-owned.
    pub ads_camera_position_ws: Option<Vec3>,
    /// Small body-forward clearance from the stable eye center. It is body-owned; view yaw/pitch
    /// are orientation-only and must never translate this offset around the head.
    pub forward_clearance: f32,
}

impl Default for PlayerFirstPersonCameraAnchor {
    #[inline]
    fn default() -> Self {
        Self {
            eye_center_ws: Vec3::ZERO,
            ads_camera_position_ws: None,
            forward_clearance: 0.045,
        }
    }
}

/// Authorable local-owner first-person presentation envelope. Offsets are expressed in the player
/// body frame relative to the stable eye anchor (engine forward = -Z). Owner geometry visibility
/// may consume the envelope, while camera position stays rigidly eye-anchored; the downward pitch
/// bound prevents exposing camera-near body cuts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerFirstPersonBodyBarrierProfile {
    pub enabled: bool,
    pub head_center_offset_ls: Vec3,
    pub head_radius: f32,
    pub neck_top_offset_ls: Vec3,
    pub neck_bottom_offset_ls: Vec3,
    pub neck_radius: f32,
    pub chest_top_offset_ls: Vec3,
    pub chest_bottom_offset_ls: Vec3,
    pub chest_radius: f32,
    pub surface_padding: f32,
    /// Maximum downward view pitch from the horizon. Upward pitch keeps the motor-authored limit.
    pub downward_pitch_limit_radians: f32,
}

impl PlayerFirstPersonBodyBarrierProfile {
    #[inline]
    pub fn from_body(body: CharacterBody) -> Self {
        let body = body.sanitized();
        let height_scale = (body.visual_half_height / 0.90).clamp(0.65, 1.60);
        let radius = body.visual_radius;
        Self {
            enabled: true,
            // +Z is behind the eyes. Keeping the primitive centre slightly behind the anchor
            // makes the safe projection resolve toward the face/front, not through the skull.
            head_center_offset_ls: Vec3::new(0.0, -0.035 * height_scale, 0.035 * height_scale),
            head_radius: (radius * 0.28).clamp(0.095, 0.145),
            neck_top_offset_ls: Vec3::new(0.0, -0.115 * height_scale, 0.030 * height_scale),
            neck_bottom_offset_ls: Vec3::new(0.0, -0.245 * height_scale, 0.045 * height_scale),
            neck_radius: (radius * 0.20).clamp(0.065, 0.100),
            chest_top_offset_ls: Vec3::new(0.0, -0.275 * height_scale, 0.055 * height_scale),
            chest_bottom_offset_ls: Vec3::new(0.0, -0.525 * height_scale, 0.070 * height_scale),
            chest_radius: (radius * 0.38).clamp(0.140, 0.205),
            surface_padding: 0.012,
            downward_pitch_limit_radians: 55.0_f32.to_radians(),
        }
    }

    #[inline]
    pub fn sanitized(self, fallback_body: CharacterBody) -> Self {
        let fallback = Self::from_body(fallback_body);
        let finite_vec = |value: Vec3, default: Vec3| {
            if value.is_finite() {
                value
            } else {
                default
            }
        };
        let finite_radius = |value: f32, default: f32, lo: f32, hi: f32| {
            if value.is_finite() {
                value.clamp(lo, hi)
            } else {
                default
            }
        };
        Self {
            enabled: self.enabled,
            head_center_offset_ls: finite_vec(
                self.head_center_offset_ls,
                fallback.head_center_offset_ls,
            ),
            head_radius: finite_radius(self.head_radius, fallback.head_radius, 0.04, 0.40),
            neck_top_offset_ls: finite_vec(self.neck_top_offset_ls, fallback.neck_top_offset_ls),
            neck_bottom_offset_ls: finite_vec(
                self.neck_bottom_offset_ls,
                fallback.neck_bottom_offset_ls,
            ),
            neck_radius: finite_radius(self.neck_radius, fallback.neck_radius, 0.03, 0.30),
            chest_top_offset_ls: finite_vec(self.chest_top_offset_ls, fallback.chest_top_offset_ls),
            chest_bottom_offset_ls: finite_vec(
                self.chest_bottom_offset_ls,
                fallback.chest_bottom_offset_ls,
            ),
            chest_radius: finite_radius(self.chest_radius, fallback.chest_radius, 0.05, 0.45),
            surface_padding: finite_radius(
                self.surface_padding,
                fallback.surface_padding,
                0.0,
                0.05,
            ),
            downward_pitch_limit_radians: finite_radius(
                self.downward_pitch_limit_radians,
                fallback.downward_pitch_limit_radians,
                35.0_f32.to_radians(),
                85.0_f32.to_radians(),
            ),
        }
    }
}

impl Default for PlayerFirstPersonBodyBarrierProfile {
    #[inline]
    fn default() -> Self {
        Self::from_body(CharacterBody::default())
    }
}
