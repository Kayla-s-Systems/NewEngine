use newengine_math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct CameraNavState {
    pub(crate) framed_once: bool,
    pub(crate) framed_radius: f32,

    pub(crate) last_frame_seq: u64,

    pub(crate) last_fly_rmb: bool,

    pub(crate) last_bounds_center: Vec3,
    pub(crate) last_bounds_radius: f32,
}

impl Default for CameraNavState {
    #[inline]
    fn default() -> Self {
        Self {
            framed_once: false,
            framed_radius: 0.0,
            last_frame_seq: 0,
            last_fly_rmb: false,
            last_bounds_center: Vec3::ZERO,
            last_bounds_radius: 1.0,
        }
    }
}
