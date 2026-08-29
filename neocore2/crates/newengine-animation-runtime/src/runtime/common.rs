pub const YCD_BODY_SCHEMA_VERSION: u32 = 2;
pub const YCD_BODY_SCHEMA_VERSION_LEGACY: u32 = 1;
pub const YCD_BODY_HEADER_LEN: usize = 48;
pub const YCD_CLIP_RECORD_LEN: usize = 64;
pub const YCD_CLIP_FLAG_LOOP: u32 = 0x1;
const LOCAL_POSE_STRIDE_V1: usize = 28;
const LOCAL_POSE_STRIDE_V2: usize = 40;

#[inline]
fn vec3(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

#[inline]
fn vec3_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

#[inline]
fn quat(value: [f32; 4]) -> Quat {
    Quat::from_xyzw(value[0], value[1], value[2], value[3])
}

#[inline]
fn quat_array(value: Quat) -> [f32; 4] {
    [value.x, value.y, value.z, value.w]
}

#[inline]
fn affine_invertible(matrix: Mat4) -> bool {
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    scale.x.is_finite()
        && scale.y.is_finite()
        && scale.z.is_finite()
        && scale.x.abs() > 1.0e-8
        && scale.y.abs() > 1.0e-8
        && scale.z.abs() > 1.0e-8
        && rotation.is_finite()
        && translation.x.is_finite()
        && translation.y.is_finite()
        && translation.z.is_finite()
}
