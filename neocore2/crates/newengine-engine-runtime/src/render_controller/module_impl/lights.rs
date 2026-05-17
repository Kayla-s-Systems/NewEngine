#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_math::Mat4;
use newengine_transform::GlobalTransform;

const MAX_POINT_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct PackedLights {
    pub ambient: [f32; 4],
    pub dir_dir_intensity: [f32; 4],
    pub dir_color: [f32; 4],
    pub point_pos_range: [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_color_intensity: [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_count_pad: [f32; 4],
    pub shadow_light_mvp: Mat4,
    pub shadow_params: [f32; 4],
    pub shadow_extra: [f32; 4],
}

impl Default for PackedLights {
    #[inline]
    fn default() -> Self {
        Self {
            ambient: [0.0, 0.0, 0.0, 0.0],
            dir_dir_intensity: [0.0, -1.0, 0.0, 0.0],
            dir_color: [1.0, 1.0, 1.0, 0.0],
            point_pos_range: [[0.0; 4]; MAX_POINT_LIGHTS],
            point_color_intensity: [[0.0; 4]; MAX_POINT_LIGHTS],
            point_count_pad: [0.0; 4],
            shadow_light_mvp: Mat4::IDENTITY,
            shadow_params: [0.0; 4],
            shadow_extra: [0.0; 4],
        }
    }
}

impl PackedLights {
    pub(super) const UBO_SIZE: usize = 480;

    #[inline]
    pub(super) fn from_world(world: &newengine_ecs::World) -> Self {
        let amb = world
            .resource::<AmbientLight>()
            .copied()
            .unwrap_or_default();
        let ambient = [amb.color[0], amb.color[1], amb.color[2], amb.intensity];

        let dir = primary_directional_light(world).unwrap_or_default();
        let dir_dir_intensity = [
            dir.direction_ws[0],
            dir.direction_ws[1],
            dir.direction_ws[2],
            dir.intensity,
        ];
        let dir_color = [dir.color[0], dir.color[1], dir.color[2], 0.0];

        let mut pts: Vec<(u64, [f32; 4], [f32; 4])> = Vec::new();
        for (e, pl, gt) in world.query2::<PointLight, GlobalTransform>() {
            let m = gt.0;
            let pos = [m.w_axis.x, m.w_axis.y, m.w_axis.z, pl.range.max(1e-3)];
            let col = [pl.color[0], pl.color[1], pl.color[2], pl.intensity.max(0.0)];
            pts.push((e.stable_u64(), pos, col));
        }
        pts.sort_by(|a, b| a.0.cmp(&b.0));

        if pts.len() > MAX_POINT_LIGHTS {
            log::warn!(
                "render: point lights truncated: requested={} max={} (deterministic keep=min stable id)",
                pts.len(),
                MAX_POINT_LIGHTS
            );
        }

        let mut out = Self {
            ambient,
            dir_dir_intensity,
            dir_color,
            ..Self::default()
        };

        let n = pts.len().min(MAX_POINT_LIGHTS);
        for i in 0..n {
            out.point_pos_range[i] = pts[i].1;
            out.point_color_intensity[i] = pts[i].2;
        }
        out.point_count_pad = [n as f32, 0.0, 0.0, 0.0];

        out
    }

    /// Store the active camera world position in std140 padding already reserved
    /// by `u_point_count_pad.yzw`. This avoids expanding the stable lit UBO ABI
    /// while giving PBR shaders the real view vector instead of the old
    /// origin-based approximation.
    #[inline]
    pub fn with_camera_position(mut self, camera_position: [f32; 3]) -> Self {
        self.point_count_pad[1] = camera_position[0];
        self.point_count_pad[2] = camera_position[1];
        self.point_count_pad[3] = camera_position[2];
        self
    }

    #[inline]
    pub fn with_shadow(mut self, light_mvp: Mat4, params: [f32; 4], extra: [f32; 4]) -> Self {
        self.shadow_light_mvp = light_mvp;
        self.shadow_params = params;
        self.shadow_extra = extra;
        self
    }

    #[inline]
    pub fn write_into(&self, bytes: &mut [u8; Self::UBO_SIZE]) {
        let mut off = 160;

        fn write_vec4(dst: &mut [u8], off: &mut usize, v: [f32; 4]) {
            for i in 0..4 {
                let o = *off + i * 4;
                dst[o..o + 4].copy_from_slice(&v[i].to_ne_bytes());
            }
            *off += 16;
        }

        write_vec4(bytes, &mut off, self.ambient);
        write_vec4(bytes, &mut off, self.dir_dir_intensity);
        write_vec4(bytes, &mut off, self.dir_color);
        for i in 0..MAX_POINT_LIGHTS {
            write_vec4(bytes, &mut off, self.point_pos_range[i]);
            write_vec4(bytes, &mut off, self.point_color_intensity[i]);
        }
        write_vec4(bytes, &mut off, self.point_count_pad);
    }
}

#[inline]
pub fn primary_directional_light(world: &newengine_ecs::World) -> Option<DirectionalLight> {
    let mut best_dir: Option<(u64, DirectionalLight)> = None;
    for (e, l) in world.query::<DirectionalLight>() {
        let k = e.stable_u64();
        if best_dir.map(|(bk, _)| k < bk).unwrap_or(true) {
            best_dir = Some((k, *l));
        }
    }
    best_dir.map(|(_, l)| l)
}


#[inline]
pub fn primary_point_light(world: &newengine_ecs::World) -> Option<(PointLight, newengine_math::Vec3)> {
    let mut best: Option<(u64, PointLight, newengine_math::Vec3)> = None;
    for (e, l, gt) in world.query2::<PointLight, GlobalTransform>() {
        let k = e.stable_u64();
        let m = gt.0;
        let pos = newengine_math::Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
        if best.map(|(bk, _, _)| k < bk).unwrap_or(true) {
            best = Some((k, *l, pos));
        }
    }
    best.map(|(_, l, pos)| (l, pos))
}

#[inline]
pub(super) fn collect_lights(world: &newengine_ecs::World) -> PackedLights {
    PackedLights::from_world(world)
}
