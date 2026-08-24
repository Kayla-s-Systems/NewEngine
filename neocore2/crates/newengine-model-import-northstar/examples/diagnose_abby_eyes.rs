use std::{env, fs, path::PathBuf};

use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_import_northstar::{
    decode_geometry_lod0, decode_skeleton, ImportedJoint, PakFile,
};

fn local_matrix(j: &ImportedJoint) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::new(j.scale_ls[0], j.scale_ls[1], j.scale_ls[2]),
        Quat::from_xyzw(
            j.rotation_ls[0],
            j.rotation_ls[1],
            j.rotation_ls[2],
            j.rotation_ls[3],
        )
        .normalize_or_identity(),
        Vec3::new(j.position_ls[0], j.position_ls[1], j.position_ls[2]),
    )
}

fn build_bind_globals(joints: &[ImportedJoint]) -> Result<Vec<Mat4>, String> {
    let mut out = vec![Mat4::IDENTITY; joints.len()];
    let mut done = vec![false; joints.len()];
    let mut left = joints.len();
    while left > 0 {
        let mut progress = false;
        for (index, joint) in joints.iter().enumerate() {
            if done[index] {
                continue;
            }
            let parent = joint.parent_index.map(|v| v as usize);
            if parent.is_some_and(|p| !done[p]) {
                continue;
            }
            let local = local_matrix(joint);
            out[index] = parent.map(|p| out[p] * local).unwrap_or(local);
            done[index] = true;
            left -= 1;
            progress = true;
        }
        if !progress {
            return Err("unresolvable skeleton hierarchy".to_owned());
        }
    }
    Ok(out)
}

#[derive(Default)]
struct GroupStats {
    count: usize,
    sum: Vec3,
    min: Vec3,
    max: Vec3,
    uv_center_count: usize,
    uv_center_sum: Vec3,
    iris_count: usize,
    iris_min: Vec3,
    iris_max: Vec3,
    uv_sum: [f64; 2],
    pos_sum: [f64; 2],
    pos_axis_sq: [f64; 2],
    uv_axis_sq: [f64; 2],
    pos_uv_cross: [f64; 2],
}

impl GroupStats {
    fn new() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
            iris_min: Vec3::splat(f32::INFINITY),
            iris_max: Vec3::splat(f32::NEG_INFINITY),
            ..Self::default()
        }
    }

    fn add(&mut self, position: Vec3, uv: [f32; 2]) {
        self.count += 1;
        self.sum += position;
        self.min = self.min.min(position);
        self.max = self.max.max(position);
        let x = position.x as f64;
        let y = position.y as f64;
        let u = uv[0] as f64;
        let v = uv[1] as f64;
        self.uv_sum[0] += u;
        self.uv_sum[1] += v;
        self.pos_sum[0] += x;
        self.pos_sum[1] += y;
        self.pos_axis_sq[0] += x * x;
        self.pos_axis_sq[1] += y * y;
        self.uv_axis_sq[0] += u * u;
        self.uv_axis_sq[1] += v * v;
        self.pos_uv_cross[0] += x * u;
        self.pos_uv_cross[1] += y * v;
        let du = uv[0] - 0.5;
        let dv = uv[1] - 0.5;
        let d2 = du * du + dv * dv;
        if d2 <= 0.08 * 0.08 {
            self.uv_center_count += 1;
            self.uv_center_sum += position;
        }
        if d2 <= 0.256 * 0.256 {
            self.iris_count += 1;
            self.iris_min = self.iris_min.min(position);
            self.iris_max = self.iris_max.max(position);
        }
    }

    fn print(&self, label: &str, joint_center: Vec3) {
        if self.count == 0 {
            return;
        }
        let mean = self.sum / self.count as f32;
        let center_uv =
            (self.uv_center_count > 0).then(|| self.uv_center_sum / self.uv_center_count as f32);
        let n = self.count as f64;
        let mean_x = self.pos_sum[0] / n;
        let mean_y = self.pos_sum[1] / n;
        let mean_u = self.uv_sum[0] / n;
        let mean_v = self.uv_sum[1] / n;
        let cov_xu = self.pos_uv_cross[0] / n - mean_x * mean_u;
        let cov_yv = self.pos_uv_cross[1] / n - mean_y * mean_v;
        let var_x = (self.pos_axis_sq[0] / n - mean_x * mean_x).max(0.0);
        let var_y = (self.pos_axis_sq[1] / n - mean_y * mean_y).max(0.0);
        let var_u = (self.uv_axis_sq[0] / n - mean_u * mean_u).max(0.0);
        let var_v = (self.uv_axis_sq[1] / n - mean_v * mean_v).max(0.0);
        let corr_xu = cov_xu / (var_x * var_u).sqrt().max(1.0e-12);
        let corr_yv = cov_yv / (var_y * var_v).sqrt().max(1.0e-12);
        println!(
            "EYE_UV_ORIENTATION side={} corr_position_x_uv_u={:.6} corr_position_y_uv_v={:.6} v_flipped={}",
            label,
            corr_xu,
            corr_yv,
            corr_yv < 0.0,
        );
        println!(
            "EYE_GEOMETRY side={} vertices={} min={:?} max={:?} mean={:?} joint_center={:?} mean_minus_joint={:?} uv_center_vertices={} uv_center_mean={:?} uv_center_minus_joint={:?}",
            label,
            self.count,
            self.min,
            self.max,
            mean,
            joint_center,
            mean - joint_center,
            self.uv_center_count,
            center_uv,
            center_uv.map(|v| v - joint_center),
        );
        if self.iris_count > 0 {
            let span = self.iris_max - self.iris_min;
            println!(
                "EYE_IRIS_PROJECTION side={} vertices={} min={:?} max={:?} span={:?} xy_aspect={:.6}",
                label, self.iris_count, self.iris_min, self.iris_max, span,
                if span.y.abs() > 1.0e-8 { span.x / span.y } else { f32::INFINITY }
            );
        }
    }
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let skeleton_path = PathBuf::from(
        args.next()
            .ok_or("usage: diagnose_abby_eyes SKELETON.pak GEOMETRY.pak...")?,
    );
    let package_paths = args.map(PathBuf::from).collect::<Vec<_>>();
    if package_paths.is_empty() {
        return Err("at least one geometry PAK is required".to_owned());
    }

    let skeleton_bytes = fs::read(&skeleton_path)
        .map_err(|e| format!("failed to read {}: {e}", skeleton_path.display()))?;
    let skeleton_pak = PakFile::parse(skeleton_bytes)?;
    let skeleton = decode_skeleton(&skeleton_pak)?;
    let globals = build_bind_globals(&skeleton.joints)?;
    let left_eye = skeleton
        .joints
        .iter()
        .position(|j| j.name == "l_eyeball")
        .ok_or("missing l_eyeball")?;
    let right_eye = skeleton
        .joints
        .iter()
        .position(|j| j.name == "r_eyeball")
        .ok_or("missing r_eyeball")?;

    println!("EYE_JOINTS left={} right={}", left_eye, right_eye);
    for index in [left_eye, right_eye] {
        let j = &skeleton.joints[index];
        let center = globals[index].transform_point3(Vec3::ZERO);
        let forward_x = globals[index]
            .transform_vector3(Vec3::X)
            .normalize_or_zero();
        let forward_y = globals[index]
            .transform_vector3(Vec3::Y)
            .normalize_or_zero();
        let forward_z = globals[index]
            .transform_vector3(Vec3::Z)
            .normalize_or_zero();
        println!(
            "EYE_BIND index={} name={} parent={:?} parent_name={:?} T={:?} Q={:?} S={:?} global_center={:?} axes[X={:?} Y={:?} Z={:?}]",
            index,
            j.name,
            j.parent_index,
            j.parent_index.map(|p| skeleton.joints[p as usize].name.as_str()),
            j.position_ls,
            j.rotation_ls,
            j.scale_ls,
            center,
            forward_x,
            forward_y,
            forward_z,
        );
    }
    for (index, joint) in skeleton.joints.iter().enumerate().take(12) {
        println!(
            "BIND_CHAIN index={} name={} parent={:?}",
            index, joint.name, joint.parent_index
        );
    }

    let left_center = globals[left_eye].transform_point3(Vec3::ZERO);
    let right_center = globals[right_eye].transform_point3(Vec3::ZERO);
    let mut global_mesh_index = 0usize;
    for path in package_paths {
        let bytes =
            fs::read(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let pak = PakFile::parse(bytes)?;
        let decoded = decode_geometry_lod0(&pak)?;
        println!("PACKAGE {} meshes={}", path.display(), decoded.meshes.len());
        for mesh in decoded.meshes {
            let mut uv_min = [f32::INFINITY; 2];
            let mut uv_max = [f32::NEG_INFINITY; 2];
            let mut uv_non_finite = 0usize;
            for v in &mesh.vertices {
                for axis in 0..2 {
                    if v.uv0[axis].is_finite() {
                        uv_min[axis] = uv_min[axis].min(v.uv0[axis]);
                        uv_max[axis] = uv_max[axis].max(v.uv0[axis]);
                    } else {
                        uv_non_finite += 1;
                    }
                }
            }
            let span = [uv_max[0] - uv_min[0], uv_max[1] - uv_min[1]];
            let uv_aspect = if span[1].abs() > 1.0e-8 {
                span[0] / span[1]
            } else {
                f32::INFINITY
            };
            let mut left_dominant = 0usize;
            let mut right_dominant = 0usize;
            let mut eye_weighted = 0usize;
            let mut left_stats = GroupStats::new();
            let mut right_stats = GroupStats::new();
            if let Some(skin) = mesh.skin.as_ref() {
                for (vertex_index, sv) in skin.iter().enumerate() {
                    let mut lw = 0.0f32;
                    let mut rw = 0.0f32;
                    for (&joint, &weight) in sv
                        .joints
                        .iter()
                        .chain(sv.joints_extra.iter())
                        .zip(sv.weights.iter().chain(sv.weights_extra.iter()))
                    {
                        if joint as usize == left_eye {
                            lw += weight;
                        }
                        if joint as usize == right_eye {
                            rw += weight;
                        }
                    }
                    if lw + rw > 0.001 {
                        eye_weighted += 1;
                    }
                    let v = &mesh.vertices[vertex_index];
                    let p = Vec3::new(v.position[0], v.position[1], v.position[2]);
                    if lw > 0.5 {
                        left_dominant += 1;
                        left_stats.add(p, v.uv0);
                    }
                    if rw > 0.5 {
                        right_dominant += 1;
                        right_stats.add(p, v.uv0);
                    }
                }
            }
            let material = mesh.source_material.as_deref().unwrap_or("<none>");
            let eye_candidate = material.to_ascii_lowercase().contains("eye")
                || mesh.name.to_ascii_lowercase().contains("eye")
                || eye_weighted > 0;
            println!(
                "MESH index={} eye_candidate={} name='{}' material='{}' vertices={} uv_min={:?} uv_max={:?} uv_span={:?} uv_aspect={:.6} uv_non_finite={} bounds_min={:?} bounds_max={:?} eye_weighted={} left_dominant={} right_dominant={}",
                global_mesh_index,
                eye_candidate,
                mesh.name,
                material,
                mesh.vertices.len(),
                uv_min,
                uv_max,
                span,
                uv_aspect,
                uv_non_finite,
                mesh.bounds_min,
                mesh.bounds_max,
                eye_weighted,
                left_dominant,
                right_dominant,
            );
            if eye_weighted > 0 {
                left_stats.print("left", left_center);
                right_stats.print("right", right_center);
            }
            global_mesh_index += 1;
        }
    }
    Ok(())
}
