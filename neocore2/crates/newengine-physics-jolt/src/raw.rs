use joltc_sys as sys;

#[inline]
pub(crate) fn vec3(v: [f32; 3]) -> sys::JPC_Vec3 {
    sys::JPC_Vec3 { x: v[0], y: v[1], z: v[2], _w: 0.0 }
}

#[inline]
pub(crate) fn float3(v: [f32; 3]) -> sys::JPC_Float3 {
    sys::JPC_Float3 { x: v[0], y: v[1], z: v[2] }
}

#[inline]
pub(crate) fn rvec3(v: [f32; 3]) -> sys::JPC_RVec3 {
    sys::JPC_RVec3 {
        x: v[0] as sys::Real,
        y: v[1] as sys::Real,
        z: v[2] as sys::Real,
        _w: 0.0 as sys::Real,
    }
}

#[inline]
pub(crate) fn quat(q: [f32; 4]) -> sys::JPC_Quat {
    sys::JPC_Quat { x: q[0], y: q[1], z: q[2], w: q[3] }
}

#[inline]
pub(crate) fn arr_from_vec3(v: sys::JPC_Vec3) -> [f32; 3] { [v.x, v.y, v.z] }

#[inline]
pub(crate) fn arr_from_rvec3(v: sys::JPC_RVec3) -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] }

#[inline]
pub(crate) fn arr_from_quat(q: sys::JPC_Quat) -> [f32; 4] { [q.x, q.y, q.z, q.w] }

#[inline]
pub(crate) fn sanitize_vec3(v: [f32; 3], min_abs: f32) -> [f32; 3] {
    [v[0].abs().max(min_abs), v[1].abs().max(min_abs), v[2].abs().max(min_abs)]
}
