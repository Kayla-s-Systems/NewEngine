#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_math::{Mat4, Vec3};

use crate::{ShadowFrame, MAX_DIRECTIONAL_SHADOW_CASCADES};

pub const MAX_POINT_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct PointLightSnapshot {
    pub stable_id: u64,
    pub light: PointLight,
    pub position: Vec3,
}

#[derive(Clone, Debug)]
pub struct LightSceneSnapshot {
    pub ambient: AmbientLight,
    pub directional: Option<DirectionalLight>,
    pub point_lights: Vec<PointLightSnapshot>,
}

impl Default for LightSceneSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            ambient: AmbientLight::default(),
            directional: None,
            point_lights: Vec::new(),
        }
    }
}

impl LightSceneSnapshot {
    #[inline]
    pub fn has_directional_light(&self) -> bool {
        self.directional.is_some()
    }

    #[inline]
    pub fn primary_directional_light(&self) -> Option<DirectionalLight> {
        self.directional
    }

    #[inline]
    pub fn primary_point_light(&self) -> Option<(PointLight, Vec3)> {
        self.point_lights
            .iter()
            .min_by_key(|p| p.stable_id)
            .map(|p| (p.light, p.position))
    }

    #[inline]
    pub fn sorted_point_lights(&self) -> Vec<PointLightSnapshot> {
        let mut pts = self.point_lights.clone();
        pts.sort_by(|a, b| a.stable_id.cmp(&b.stable_id));
        pts
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PackedLights {
    pub ambient: [f32; 4],
    pub dir_dir_intensity: [f32; 4],
    pub dir_color: [f32; 4],
    pub point_pos_range: [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_color_intensity: [[f32; 4]; MAX_POINT_LIGHTS],
    pub point_count_pad: [f32; 4],
    pub shadow_light_mvp: Mat4,
    pub shadow_cascade_light_mvp: [Mat4; MAX_DIRECTIONAL_SHADOW_CASCADES],
    pub shadow_cascade_splits: [f32; MAX_DIRECTIONAL_SHADOW_CASCADES],
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
            shadow_cascade_light_mvp: [Mat4::IDENTITY; MAX_DIRECTIONAL_SHADOW_CASCADES],
            shadow_cascade_splits: [0.0; MAX_DIRECTIONAL_SHADOW_CASCADES],
            shadow_params: [0.0; 4],
            shadow_extra: [0.0; 4],
        }
    }
}

impl PackedLights {
    pub const UBO_SIZE: usize = 752;

    #[inline]
    pub fn from_snapshot(snapshot: &LightSceneSnapshot) -> Self {
        let amb = snapshot.ambient;
        let ambient = [amb.color[0], amb.color[1], amb.color[2], amb.intensity];

        let dir = snapshot.primary_directional_light().unwrap_or_default();
        let dir_dir_intensity = [
            dir.direction_ws[0],
            dir.direction_ws[1],
            dir.direction_ws[2],
            dir.intensity,
        ];
        let dir_color = [dir.color[0], dir.color[1], dir.color[2], 0.0];

        let pts = snapshot.sorted_point_lights();
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
            let p = pts[i];
            out.point_pos_range[i] = [
                p.position.x,
                p.position.y,
                p.position.z,
                p.light.range.max(1e-3),
            ];
            out.point_color_intensity[i] = [
                p.light.color[0],
                p.light.color[1],
                p.light.color[2],
                p.light.intensity.max(0.0),
            ];
        }
        out.point_count_pad = [n as f32, 0.0, 0.0, 0.0];
        out
    }

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
        self.shadow_cascade_light_mvp = [light_mvp; MAX_DIRECTIONAL_SHADOW_CASCADES];
        self.shadow_cascade_splits = [extra[3].max(0.0); MAX_DIRECTIONAL_SHADOW_CASCADES];
        self.shadow_params = params;
        self.shadow_extra = extra;
        self
    }

    #[inline]
    pub fn with_shadow_frame(mut self, frame: ShadowFrame) -> Self {
        self.shadow_light_mvp = frame.light_mvp;
        self.shadow_cascade_light_mvp = frame.cascade_light_mvp;
        self.shadow_cascade_splits = frame.cascade_splits;
        self.shadow_params = frame.params;
        self.shadow_extra = frame.extra;
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
