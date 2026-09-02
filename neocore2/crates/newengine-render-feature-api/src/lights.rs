#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{
    AmbientLight, DirectionalLight, PointLight, SpotLight, SOLAR_ANGULAR_RADIUS_RADIANS,
};
use newengine_math::{Mat4, Vec3};

use crate::{
    LocalShadowFrame, ShadowFrame, MAX_DIRECTIONAL_SHADOW_CASCADES, MAX_LOCAL_SHADOW_LIGHTS,
    MAX_LOCAL_SHADOW_VIEWS,
};

pub const MAX_POINT_LIGHTS: usize = 4;
pub const MAX_SPOT_LIGHTS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct PointLightSnapshot {
    pub stable_id: u64,
    pub light: PointLight,
    pub position: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct SpotLightSnapshot {
    pub stable_id: u64,
    pub light: SpotLight,
    pub position: Vec3,
}

#[derive(Clone, Debug)]
pub struct LightSceneSnapshot {
    pub ambient: AmbientLight,
    pub directional: Option<DirectionalLight>,
    pub point_lights: Vec<PointLightSnapshot>,
    pub spot_lights: Vec<SpotLightSnapshot>,
}

impl Default for LightSceneSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            ambient: AmbientLight::default(),
            directional: None,
            point_lights: Vec::new(),
            spot_lights: Vec::new(),
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
        pts.sort_by_key(|light| light.stable_id);
        pts
    }

    #[inline]
    pub fn primary_spot_light(&self) -> Option<(SpotLight, Vec3)> {
        self.spot_lights
            .iter()
            .min_by_key(|s| s.stable_id)
            .map(|s| (s.light, s.position))
    }

    #[inline]
    pub fn sorted_spot_lights(&self) -> Vec<SpotLightSnapshot> {
        let mut spots = self.spot_lights.clone();
        spots.sort_by_key(|light| light.stable_id);
        spots
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
    pub spot_pos_range: [[f32; 4]; MAX_SPOT_LIGHTS],
    pub spot_dir_outer_cos: [[f32; 4]; MAX_SPOT_LIGHTS],
    pub spot_color_intensity: [[f32; 4]; MAX_SPOT_LIGHTS],
    pub spot_inner_cos: [f32; 4],
    pub spot_count_pad: [f32; 4],
    pub local_shadow_mvp: [Mat4; MAX_LOCAL_SHADOW_VIEWS],
    /// xy = atlas scale, zw = atlas offset for each local shadow view.
    pub local_shadow_tile: [[f32; 4]; MAX_LOCAL_SHADOW_VIEWS],
    /// x=enabled, y=first view index, z=depth bias, w=normal-bias scale.
    pub point_shadow_meta: [[f32; 4]; MAX_POINT_LIGHTS],
    /// x=enabled, y=first view index, z=depth bias, w=normal-bias scale.
    pub spot_shadow_meta: [[f32; 4]; MAX_SPOT_LIGHTS],
    /// x=enabled, y=view count, z=global visibility strength, w=reserved.
    pub local_shadow_atlas: [f32; 4],
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
    /// Sky cloud physical profile, append-only std140 lanes.
    /// profile0 = [low_base_m, low_thickness_m, low_density, high_coverage]
    pub sky_cloud_profile0: [f32; 4],
    /// profile1 = [humidity, aerosol_density, precipitation, high_density]
    pub sky_cloud_profile1: [f32; 4],
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
            spot_pos_range: [[0.0; 4]; MAX_SPOT_LIGHTS],
            spot_dir_outer_cos: [[0.0; 4]; MAX_SPOT_LIGHTS],
            spot_color_intensity: [[0.0; 4]; MAX_SPOT_LIGHTS],
            spot_inner_cos: [0.0; 4],
            spot_count_pad: [0.0; 4],
            local_shadow_mvp: [Mat4::IDENTITY; MAX_LOCAL_SHADOW_VIEWS],
            local_shadow_tile: [[0.0; 4]; MAX_LOCAL_SHADOW_VIEWS],
            point_shadow_meta: [[0.0; 4]; MAX_POINT_LIGHTS],
            spot_shadow_meta: [[0.0; 4]; MAX_SPOT_LIGHTS],
            local_shadow_atlas: [0.0; 4],
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
            sky_cloud_profile0: [1250.0, 1100.0, 0.16, 0.08],
            sky_cloud_profile1: [0.45, 0.12, 0.0, 0.04],
            shadow_view_forward: [0.0, 0.0, 1.0, 0.0],
        }
    }
}

impl PackedLights {
    pub const UBO_SIZE: usize = 3200;

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
        // Alpha is an append-safe physical source lane: shaders use it as the solar
        // angular half-radius. RGB remains the linear directional-light chromaticity.
        let dir_color = [
            dir.color[0],
            dir.color[1],
            dir.color[2],
            SOLAR_ANGULAR_RADIUS_RADIANS,
        ];

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

        let spots = snapshot.sorted_spot_lights();
        if spots.len() > MAX_SPOT_LIGHTS {
            newengine_ulog_api::ulog::warn!(
                "render: spot lights truncated: requested={} max={} (deterministic keep=min stable id)",
                spots.len(),
                MAX_SPOT_LIGHTS
            );
        }
        let spot_n = spots.len().min(MAX_SPOT_LIGHTS);
        for (i, s) in spots.iter().enumerate().take(spot_n) {
            let direction = Vec3::new(
                s.light.direction_ws[0],
                s.light.direction_ws[1],
                s.light.direction_ws[2],
            )
            .normalize_or_zero();
            let outer_angle = s.light.outer_angle_rad.clamp(0.01, 1.553_343);
            let inner_angle = s.light.inner_angle_rad.clamp(0.0, outer_angle);
            out.spot_pos_range[i] = [
                s.position.x,
                s.position.y,
                s.position.z,
                s.light.range.max(1.0e-3),
            ];
            out.spot_dir_outer_cos[i] = [direction.x, direction.y, direction.z, outer_angle.cos()];
            out.spot_color_intensity[i] = [
                s.light.color[0],
                s.light.color[1],
                s.light.color[2],
                s.light.intensity.max(0.0),
            ];
            out.spot_inner_cos[i] = inner_angle.cos();
        }
        out.spot_count_pad = [spot_n as f32, 0.0, 0.0, 0.0];
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
    pub fn with_sky_cloud_profile(mut self, profile0: [f32; 4], profile1: [f32; 4]) -> Self {
        self.sky_cloud_profile0 = profile0;
        self.sky_cloud_profile1 = profile1;
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
    pub fn with_local_shadow_frame(mut self, frame: LocalShadowFrame) -> Self {
        self.local_shadow_mvp = [Mat4::IDENTITY; MAX_LOCAL_SHADOW_VIEWS];
        self.local_shadow_tile = [[0.0; 4]; MAX_LOCAL_SHADOW_VIEWS];
        self.point_shadow_meta = [[0.0; 4]; MAX_POINT_LIGHTS];
        self.spot_shadow_meta = [[0.0; 4]; MAX_SPOT_LIGHTS];
        self.local_shadow_atlas = [0.0; 4];
        if !frame.is_active() {
            return self;
        }

        let view_count = frame.view_count.min(MAX_LOCAL_SHADOW_VIEWS as u32) as usize;
        for i in 0..view_count {
            let view = frame.views[i];
            self.local_shadow_mvp[i] = view.light_mvp;
            self.local_shadow_tile[i] = view.atlas_uv_transform(frame.atlas_extent);
        }
        let light_count = frame.light_count.min(MAX_LOCAL_SHADOW_LIGHTS as u32) as usize;
        let mut max_strength = 0.0_f32;
        for i in 0..light_count {
            let light = frame.lights[i];
            let meta = [
                1.0,
                light.first_view as f32,
                light.bias.max(0.0),
                light.normal_bias.max(0.0),
            ];
            max_strength = max_strength.max(light.strength.clamp(0.0, 1.0));
            match light.light_kind {
                crate::ShadowLightKind::Point => {
                    let index = light.packed_light_index as usize;
                    if index < MAX_POINT_LIGHTS {
                        self.point_shadow_meta[index] = meta;
                    }
                }
                crate::ShadowLightKind::Spot => {
                    let index = light.packed_light_index as usize;
                    if index < MAX_SPOT_LIGHTS {
                        self.spot_shadow_meta[index] = meta;
                    }
                }
                crate::ShadowLightKind::Directional => {}
            }
        }
        self.local_shadow_atlas = [1.0, view_count as f32, max_strength, 0.0];
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

        // Append local-light data after the legacy 880-byte block. Keeping the
        // original offsets intact preserves compatibility with shaders that do not
        // consume spot lights yet.
        let mut spot_off = 880usize;
        for value in self.spot_pos_range {
            write_vec4(bytes, &mut spot_off, value);
        }
        for value in self.spot_dir_outer_cos {
            write_vec4(bytes, &mut spot_off, value);
        }
        for value in self.spot_color_intensity {
            write_vec4(bytes, &mut spot_off, value);
        }
        write_vec4(bytes, &mut spot_off, self.spot_inner_cos);
        write_vec4(bytes, &mut spot_off, self.spot_count_pad);

        let mut local_off = 1104usize;
        for matrix in self.local_shadow_mvp {
            for component in matrix.to_cols_array() {
                bytes[local_off..local_off + 4].copy_from_slice(&component.to_ne_bytes());
                local_off += 4;
            }
        }
        for value in self.local_shadow_tile {
            write_vec4(bytes, &mut local_off, value);
        }
        for value in self.point_shadow_meta {
            write_vec4(bytes, &mut local_off, value);
        }
        for value in self.spot_shadow_meta {
            write_vec4(bytes, &mut local_off, value);
        }
        write_vec4(bytes, &mut local_off, self.local_shadow_atlas);
        write_vec4(bytes, &mut local_off, self.sky_cloud_profile0);
        write_vec4(bytes, &mut local_off, self.sky_cloud_profile1);
        debug_assert_eq!(local_off, Self::UBO_SIZE);
    }
}

#[cfg(test)]
mod cloud_shadow_ubo_tests {
    use super::*;

    #[test]
    fn packed_camera_forward_is_normalized_for_csm_receiver_depth() {
        let packed = PackedLights::default().with_camera_forward([0.0, 3.0, 4.0]);
        assert_eq!(PackedLights::UBO_SIZE, 3200);
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
        assert_eq!(PackedLights::UBO_SIZE, 3200);
        assert_eq!(packed.shadow_pcss0, pcss0);
        assert_eq!(packed.shadow_pcss1, pcss1);
    }

    #[test]
    fn packed_sky_cloud_profile_occupies_tail_slots() {
        let profile0 = [920.0, 1840.0, 0.62, 0.31];
        let profile1 = [0.78, 0.24, 0.18, 0.12];
        let packed = PackedLights::default().with_sky_cloud_profile(profile0, profile1);
        let mut bytes = [0u8; PackedLights::UBO_SIZE];
        packed.write_into(&mut bytes);
        let read_f32 = |offset: usize| {
            f32::from_ne_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("four byte float"),
            )
        };
        assert_eq!(PackedLights::UBO_SIZE, 3200);
        assert_eq!(packed.sky_cloud_profile0, profile0);
        assert_eq!(packed.sky_cloud_profile1, profile1);
        assert_eq!(read_f32(3168), profile0[0]);
        assert_eq!(read_f32(3184), profile1[0]);
    }

    #[test]
    fn packed_cloud_shadow_occupies_appended_std140_slots() {
        let map0 = [0.11, 0.22, 0.33, 0.44];
        let map1 = [0.005, 1800.0, 0.55, 0.66];
        let map2 = [0.77, 0.28, 0.82, 1.0];
        let map3 = [0.10, 0.20, 0.31, 0.43];
        let map4 = [0.78, 0.035, 0.17, 96.0];
        let packed = PackedLights::default().with_cloud_shadow(map0, map1, map2, map3, map4);
        assert_eq!(PackedLights::UBO_SIZE, 3200);
        assert_eq!(packed.cloud_shadow_map0, map0);
        assert_eq!(packed.cloud_shadow_map1, map1);
        assert_eq!(packed.cloud_shadow_map2, map2);
        assert_eq!(packed.cloud_shadow_map3, map3);
        assert_eq!(packed.cloud_shadow_map4, map4);
    }
}
