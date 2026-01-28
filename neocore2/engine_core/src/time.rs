#[derive(Debug, Clone)]
pub struct Time {
    pub dt_sec: f32,
    pub t_sec: f64,
    pub frame_index: u64,

    pub fixed_tick_index: u64,
    pub fixed_alpha: f32,
    pub fixed_dt_sec: f32,
}

impl Time {
    pub fn new(fixed_dt_sec: f32) -> Self {
        Self {
            dt_sec: 0.0,
            t_sec: 0.0,
            frame_index: 0,
            fixed_tick_index: 0,
            fixed_alpha: 0.0,
            fixed_dt_sec,
        }
    }
}