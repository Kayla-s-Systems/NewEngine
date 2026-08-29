use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisibilitySettings {
    #[serde(default)]
    pub gpu_visibility_enabled: bool,
    #[serde(default = "default_true")]
    pub hiz_enabled: bool,
    #[serde(default = "default_true")]
    pub pvs_sort_enabled: bool,
    #[serde(default = "default_true")]
    pub zone_cull_enabled: bool,
}

impl Default for VisibilitySettings {
    #[inline]
    fn default() -> Self {
        Self {
            gpu_visibility_enabled: false,
            hiz_enabled: true,
            pvs_sort_enabled: true,
            zone_cull_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FrameCameraContext {
    #[serde(default)]
    pub position_ws: [f32; 3],
    #[serde(default = "default_camera_forward")]
    pub forward_ws: [f32; 3],
    #[serde(default = "default_camera_up")]
    pub up_ws: [f32; 3],
    #[serde(default = "default_camera_fov_y")]
    pub fov_y: f32,
    #[serde(default = "default_camera_near")]
    pub near: f32,
    #[serde(default = "default_camera_far")]
    pub far: f32,
}

impl Default for FrameCameraContext {
    #[inline]
    fn default() -> Self {
        Self {
            position_ws: [0.0, 0.0, 0.0],
            forward_ws: default_camera_forward(),
            up_ws: default_camera_up(),
            fov_y: default_camera_fov_y(),
            near: default_camera_near(),
            far: default_camera_far(),
        }
    }
}

impl FrameCameraContext {
    #[inline]
    pub fn shadow_cache_bucket_hash(self) -> u32 {
        const POS_STEP_METERS: f32 = 0.5;
        const ANGLE_STEP_DEGREES: f32 = 1.0;

        fn quantize_position(value: f32) -> i32 {
            if !value.is_finite() {
                return 0;
            }
            (value / POS_STEP_METERS)
                .round()
                .clamp(i32::MIN as f32, i32::MAX as f32) as i32
        }

        fn normalize_or_default(v: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
            let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            if len_sq <= 1.0e-8 || !len_sq.is_finite() {
                return fallback;
            }
            let inv_len = len_sq.sqrt().recip();
            [v[0] * inv_len, v[1] * inv_len, v[2] * inv_len]
        }

        fn quantize_unit_component(value: f32) -> i16 {
            let degrees = value.clamp(-1.0, 1.0).asin().to_degrees();
            (degrees / ANGLE_STEP_DEGREES)
                .round()
                .clamp(i16::MIN as f32, i16::MAX as f32) as i16
        }

        fn mix(hash: &mut u32, value: u32) {
            *hash ^= value;
            *hash = hash.wrapping_mul(0x0100_0193);
        }

        let forward = normalize_or_default(self.forward_ws, default_camera_forward());
        let up = normalize_or_default(self.up_ws, default_camera_up());
        let mut hash = 0x811C_9DC5_u32;
        for value in [
            quantize_position(self.position_ws[0]) as u32,
            quantize_position(self.position_ws[1]) as u32,
            quantize_position(self.position_ws[2]) as u32,
            quantize_unit_component(forward[0]) as u32,
            quantize_unit_component(forward[1]) as u32,
            quantize_unit_component(forward[2]) as u32,
            quantize_unit_component(up[0]) as u32,
            quantize_unit_component(up[1]) as u32,
            quantize_unit_component(up[2]) as u32,
        ] {
            mix(&mut hash, value);
        }
        hash
    }
}

#[inline]
fn default_true() -> bool {
    true
}
#[inline]
fn default_camera_forward() -> [f32; 3] {
    [0.0, 0.0, -1.0]
}
#[inline]
fn default_camera_up() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}
#[inline]
fn default_camera_fov_y() -> f32 {
    60.0_f32.to_radians()
}
#[inline]
fn default_camera_near() -> f32 {
    0.05
}
#[inline]
fn default_camera_far() -> f32 {
    10_000.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondaryCommandBufferSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub shadow_cascades: bool,
    #[serde(default = "default_true")]
    pub postfx_passes: bool,
    #[serde(default = "default_true")]
    pub visibility_compute: bool,
    #[serde(default = "default_true")]
    pub water_reflection_scopes: bool,
}

impl Default for SecondaryCommandBufferSettings {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            shadow_cascades: true,
            postfx_passes: true,
            visibility_compute: true,
            water_reflection_scopes: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePacketBridgeSettings {
    #[serde(default)]
    pub entity_packets_ready: bool,
    #[serde(default)]
    pub light_packets_ready: bool,
    #[serde(default)]
    pub vegetation_instance_packets_ready: bool,
    #[serde(default)]
    pub reflection_zone_packets_ready: bool,
    #[serde(default)]
    pub visibility_object_bound_packets_ready: bool,
}

impl Default for RuntimePacketBridgeSettings {
    #[inline]
    fn default() -> Self {
        Self {
            entity_packets_ready: false,
            light_packets_ready: false,
            vegetation_instance_packets_ready: false,
            reflection_zone_packets_ready: false,
            visibility_object_bound_packets_ready: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindlessDescriptorSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bindless_texture_capacity")]
    pub texture_capacity: u32,
    #[serde(default = "default_bindless_material_capacity")]
    pub material_capacity: u32,
    #[serde(default = "default_true")]
    pub vegetation_textures: bool,
    #[serde(default = "default_true")]
    pub instanced_materials: bool,
    #[serde(default = "default_true")]
    pub postfx_texture_chain: bool,
}

impl Default for BindlessDescriptorSettings {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            texture_capacity: default_bindless_texture_capacity(),
            material_capacity: default_bindless_material_capacity(),
            vegetation_textures: true,
            instanced_materials: true,
            postfx_texture_chain: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RendererParitySettings {
    #[serde(default)]
    pub secondary_command_buffers: SecondaryCommandBufferSettings,
    #[serde(default)]
    pub runtime_packets: RuntimePacketBridgeSettings,
    #[serde(default)]
    pub bindless: BindlessDescriptorSettings,
}

impl Default for RendererParitySettings {
    #[inline]
    fn default() -> Self {
        Self {
            secondary_command_buffers: SecondaryCommandBufferSettings::default(),
            runtime_packets: RuntimePacketBridgeSettings::default(),
            bindless: BindlessDescriptorSettings::default(),
        }
    }
}

#[inline]
fn default_bindless_texture_capacity() -> u32 {
    16_384
}
#[inline]
fn default_bindless_material_capacity() -> u32 {
    8_192
}
