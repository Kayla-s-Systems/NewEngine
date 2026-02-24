#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Mat4, Quat, Vec2, Vec3};

use crate::{CameraMatrices, CameraRig, Frustum, Projection};

/// Input for the universal camera stack.
///
/// This is intentionally game-leaning and does not overlap with `CameraInput` (editor controllers).
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraStackInput {
    /// Delta time for the current tick/frame.
    pub dt: f32,

    /// Determinism seed for procedural effects.
    pub seed: u64,

    /// Raw mouse delta (or right-stick delta) in pixels per frame.
    pub look_delta: Vec2,

    /// Normalized aim state.
    pub is_aiming: bool,

    /// Whether the character is grounded.
    pub is_grounded: bool,

    /// World-space linear velocity of the camera anchor (usually the character).
    pub velocity_ws: Vec3,

    /// Additional user-driven intensity scalar.
    pub intensity: f32,

    /// Optional world-space anchor (character head, camera pivot, etc.).
    /// When `has_anchor` is true, the stack starts each update from this pose.
    pub has_anchor: bool,
    pub anchor_pos_ws: Vec3,
    pub anchor_rot_ws: Quat,
}

/// World-space delta for camera pose.
#[derive(Clone, Copy, Debug)]
pub struct PoseDelta {
    pub dpos_ws: Vec3,
    /// Local-space rotation delta applied on top of current rig rotation.
    pub drot_ls: Quat,
}

impl Default for PoseDelta {
    #[inline]
    fn default() -> Self {
        Self {
            dpos_ws: Vec3::ZERO,
            drot_ls: Quat::IDENTITY,
        }
    }
}

/// Projection-space delta.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectionDelta {
    /// Jitter in pixels.
    pub jitter_px: Vec2,
    /// Additive vertical FOV delta (radians). Intended for breathing, recoil ADS transitions, etc.
    pub fovy_add: f32,
}

/// Output of a modifier application.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModifierOutput {
    pub pose: PoseDelta,
    pub proj: ProjectionDelta,
}

/// A deterministic camera modifier.
///
/// Modifiers are applied in order. Each modifier sees the current state produced by previous ones.
pub trait CameraModifier: Send + Sync {
    fn apply(
        &mut self,
        rig: &CameraRig,
        proj: &Projection,
        input: &CameraStackInput,
    ) -> ModifierOutput;
}

/// Universal camera rig pipeline.
///
/// - Owns `CameraRig` + `Projection`.
/// - Applies a stack of modifiers (gameplay + editor post effects).
/// - Produces matrices and frustum each frame.
pub struct CameraStack {
    pub rig: CameraRig,
    pub projection: Projection,

    pub viewport_wh: Vec2,

    modifiers: Vec<Box<dyn CameraModifier>>,
}

impl Default for CameraStack {
    fn default() -> Self {
        Self {
            rig: CameraRig::default(),
            projection: Projection::Perspective(crate::Perspective::new(
                60.0_f32.to_radians(),
                16.0 / 9.0,
                0.01,
                10_000.0,
            )),
            viewport_wh: Vec2::new(1920.0, 1080.0),
            modifiers: Vec::new(),
        }
    }
}

impl CameraStack {
    #[inline]
    pub fn new(rig: CameraRig, projection: Projection) -> Self {
        Self {
            rig,
            projection,
            viewport_wh: Vec2::new(1920.0, 1080.0),
            modifiers: Vec::new(),
        }
    }

    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        self.viewport_wh = Vec2::new(w, h);
        self.projection.set_viewport(width, height);
    }

    #[inline]
    pub fn push_modifier(&mut self, m: Box<dyn CameraModifier>) {
        self.modifiers.push(m);
    }

    #[inline]
    pub fn clear_modifiers(&mut self) {
        self.modifiers.clear();
    }

    /// Runs the camera stack and returns matrices + frustum.
    #[inline]
    pub fn update(&mut self, input: CameraStackInput) -> (CameraMatrices, Frustum) {
        let mut rig = self.rig;
        let mut proj = self.projection;

        if input.has_anchor {
            rig.position = input.anchor_pos_ws;
            rig.rotation = input.anchor_rot_ws.normalize_or_identity();
        }

        let mut jitter_px = Vec2::ZERO;
        let mut fovy_add = 0.0f32;

        for m in &mut self.modifiers {
            let out = m.apply(&rig, &proj, &input);

            rig.position += out.pose.dpos_ws;
            rig.rotation = (rig.rotation * out.pose.drot_ls).normalize_or_identity();

            jitter_px += out.proj.jitter_px;
            fovy_add += out.proj.fovy_add;

            proj = apply_fovy_add(proj, fovy_add);
        }

        let view = rig.view_matrix();
        let mut proj_m = proj.matrix();
        proj_m = apply_jitter(proj_m, jitter_px, self.viewport_wh);

        let mats = CameraMatrices::new(view, proj_m, rig.position, self.viewport_wh, jitter_px);
        let frustum = Frustum::from_view_proj(mats.view_proj);

        // Persist base state for the next tick.
        self.rig = rig;
        self.projection = proj;

        (mats, frustum)
    }
}

#[inline]
fn apply_fovy_add(proj: Projection, fovy_add: f32) -> Projection {
    match proj {
        Projection::Perspective(mut p) => {
            if fovy_add.is_finite() && fovy_add.abs() > 1e-9 {
                p.fovy = (p.fovy + fovy_add).clamp(0.01, 3.12);
            }
            Projection::Perspective(p)
        }
        x => x,
    }
}

#[inline]
fn apply_jitter(mut proj: Mat4, jitter_px: Vec2, viewport_wh: Vec2) -> Mat4 {
    let w = viewport_wh.x.max(1.0);
    let h = viewport_wh.y.max(1.0);

    let dx = (2.0 * jitter_px.x) / w;
    let dy = (2.0 * jitter_px.y) / h;

    // Jitter is defined in clip space. Therefore we must pre-multiply:
    // clip' = T * (P * V * world)
    proj = Mat4::from_translation(Vec3::new(dx, dy, 0.0)) * proj;
    proj
}
