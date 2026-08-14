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
    pub shadow_pcss0: [f32; 4],
    pub shadow_pcss1: [f32; 4],
    pub cloud_shadow_map0: [f32; 4],
    pub cloud_shadow_map1: [f32; 4],
    pub cloud_shadow_map2: [f32; 4],
    pub cloud_shadow_map3: [f32; 4],
    pub cloud_shadow_map4: [f32; 4],
    /// xyz = normalized active camera forward direction; w = receiver diagnostic mode.
    /// Appended to the std140 block so CSM receiver selection can use the exact
    /// same camera-forward depth convention as CPU cascade fitting.
    pub shadow_view_forward: [f32; 4],
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
            shadow_pcss0: [0.0; 4],
            shadow_pcss1: [0.0; 4],
            cloud_shadow_map0: [0.0; 4],
            cloud_shadow_map1: [0.0; 4],
            cloud_shadow_map2: [0.0; 4],
            cloud_shadow_map3: [0.0; 4],
            cloud_shadow_map4: [0.0; 4],
            shadow_view_forward: [0.0, 0.0, 1.0, 0.0],
        }
    }
}

impl PackedLights {
    pub const UBO_SIZE: usize = 880;

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
            newengine_ulog_api::ulog::warn!(
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
        for (i, p) in pts.iter().enumerate().take(n) {
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
    pub fn with_camera_forward(mut self, camera_forward: [f32; 3]) -> Self {
        let len2 = camera_forward[0] * camera_forward[0]
            + camera_forward[1] * camera_forward[1]
            + camera_forward[2] * camera_forward[2];
        if len2 > 1.0e-8 {
            let inv_len = len2.sqrt().recip();
            self.shadow_view_forward = [
                camera_forward[0] * inv_len,
                camera_forward[1] * inv_len,
                camera_forward[2] * inv_len,
                0.0,
            ];
        }
        self
    }

    #[inline]
    pub fn with_shadow_receiver_debug_mode(mut self, mode: f32) -> Self {
        self.shadow_view_forward[3] = mode.clamp(0.0, 8.0);
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
    pub fn with_cloud_shadow(
        mut self,
        map0: [f32; 4],
        map1: [f32; 4],
        map2: [f32; 4],
        map3: [f32; 4],
        map4: [f32; 4],
    ) -> Self {
        self.cloud_shadow_map0 = map0;
        self.cloud_shadow_map1 = map1;
        self.cloud_shadow_map2 = map2;
        self.cloud_shadow_map3 = map3;
        self.cloud_shadow_map4 = map4;
        self
    }

    #[inline]
    pub fn with_shadow_frame(mut self, frame: ShadowFrame) -> Self {
        self.shadow_light_mvp = frame.light_mvp;
        self.shadow_cascade_light_mvp = frame.cascade_light_mvp;
        self.shadow_cascade_splits = frame.cascade_splits;
        self.shadow_params = frame.params;
        self.shadow_extra = frame.extra;
        self.shadow_pcss0 = frame.pcss0;
        self.shadow_pcss1 = frame.pcss1;
        self
    }

    #[inline]
    pub fn write_into(&self, bytes: &mut [u8; Self::UBO_SIZE]) {
        let mut off = 160;
        fn write_vec4(dst: &mut [u8], off: &mut usize, v: [f32; 4]) {
            for (i, component) in v.iter().enumerate() {
                let o = *off + i * 4;
                dst[o..o + 4].copy_from_slice(&component.to_ne_bytes());
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

#[cfg(test)]
mod cloud_shadow_ubo_tests {
    use super::*;

    #[test]
    fn packed_camera_forward_is_normalized_for_csm_receiver_depth() {
        let packed = PackedLights::default().with_camera_forward([0.0, 3.0, 4.0]);
        assert_eq!(PackedLights::UBO_SIZE, 880);
        assert!((packed.shadow_view_forward[0] - 0.0).abs() < 1.0e-6);
        assert!((packed.shadow_view_forward[1] - 0.6).abs() < 1.0e-6);
        assert!((packed.shadow_view_forward[2] - 0.8).abs() < 1.0e-6);
        assert_eq!(packed.shadow_view_forward[3], 0.0);
    }

    #[test]
    fn packed_receiver_debug_mode_uses_reserved_shadow_view_slot() {
        let packed = PackedLights::default()
            .with_camera_forward([0.0, 0.0, 1.0])
            .with_shadow_receiver_debug_mode(7.0);
        assert_eq!(packed.shadow_view_forward, [0.0, 0.0, 1.0, 7.0]);
    }

    #[test]
    fn packed_pcss_parameters_survive_shadow_frame_bridge() {
        let pcss0 = [2.0, 0.00464, 5.0, 12.0];
        let pcss1 = [12.0, 16.0, 0.55, 4.0];
        let frame = ShadowFrame::disabled(newengine_core::render::TextureId::new(1))
            .with_pcss(pcss0, pcss1);
        let packed = PackedLights::default().with_shadow_frame(frame);
        assert_eq!(PackedLights::UBO_SIZE, 880);
        assert_eq!(packed.shadow_pcss0, pcss0);
        assert_eq!(packed.shadow_pcss1, pcss1);
    }

    #[test]
    fn packed_cloud_shadow_occupies_appended_std140_slots() {
        let map0 = [0.11, 0.22, 0.33, 0.44];
        let map1 = [0.005, 1800.0, 0.55, 0.66];
        let map2 = [0.77, 0.28, 0.82, 1.0];
        let map3 = [0.10, 0.20, 0.31, 0.43];
        let map4 = [0.78, 0.035, 0.17, 96.0];
        let packed = PackedLights::default().with_cloud_shadow(map0, map1, map2, map3, map4);
        assert_eq!(PackedLights::UBO_SIZE, 880);
        assert_eq!(packed.cloud_shadow_map0, map0);
        assert_eq!(packed.cloud_shadow_map1, map1);
        assert_eq!(packed.cloud_shadow_map2, map2);
        assert_eq!(packed.cloud_shadow_map3, map3);
        assert_eq!(packed.cloud_shadow_map4, map4);
    }
}
