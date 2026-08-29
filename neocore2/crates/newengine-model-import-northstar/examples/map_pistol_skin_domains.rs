use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{collections::BTreeMap, env, fs};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PositionKey(i64, i64, i64);

fn key(position: [f32; 3], quantum: f64) -> PositionKey {
    PositionKey(
        (f64::from(position[0]) / quantum).round() as i64,
        (f64::from(position[1]) / quantum).round() as i64,
        (f64::from(position[2]) / quantum).round() as i64,
    )
}

fn dominant_joint(joints: [u16; 4], weights: [f32; 4], extra_joints: [u16; 4], extra_weights: [f32; 4]) -> Option<u16> {
    let mut best = None::<(f32, u16)>;
    for (joint, weight) in joints.into_iter().zip(weights).chain(extra_joints.into_iter().zip(extra_weights)) {
        if !weight.is_finite() || weight <= 0.0 {
            continue;
        }
        if best.is_none_or(|(best_weight, best_joint)| weight > best_weight || (weight == best_weight && joint < best_joint)) {
            best = Some((weight, joint));
        }
    }
    best.map(|(_, joint)| joint)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let common_path = args.next().ok_or("usage: map_pistol_skin_domains COMMON.pak LOCAL1.pak [LOCAL2.pak ...]")?;
    let local_paths = args.collect::<Vec<_>>();
    if local_paths.is_empty() {
        return Err("at least one local parts PAK is required".to_owned());
    }

    let common = PakFile::parse(fs::read(&common_path).map_err(|e| e.to_string())?)?;
    let common_geometry = decode_geometry_lod0(&common)?;
    let quantum = 1.0e-5_f64;

    let mut common_positions = BTreeMap::<PositionKey, Vec<(u16, String, usize)>>::new();
    for mesh in &common_geometry.meshes {
        let Some(skin) = &mesh.skin else { continue };
        for (vertex_index, (vertex, skin)) in mesh.vertices.iter().zip(skin).enumerate() {
            let Some(joint) = dominant_joint(skin.joints, skin.weights, skin.joints_extra, skin.weights_extra) else { continue };
            common_positions.entry(key(vertex.position, quantum)).or_default().push((joint, mesh.name.clone(), vertex_index));
        }
    }

    println!("common='{common_path}' meshes={} unique_positions={} quantum={quantum}", common_geometry.meshes.len(), common_positions.len());

    let mut pairs = BTreeMap::<(u16, u16), u64>::new();
    let mut local_joint_totals = BTreeMap::<u16, u64>::new();
    let mut common_joint_totals = BTreeMap::<u16, u64>::new();
    let mut exact_matches = 0_u64;
    let mut ambiguous_matches = 0_u64;
    let mut unmatched = 0_u64;

    for local_path in &local_paths {
        let local = PakFile::parse(fs::read(local_path).map_err(|e| e.to_string())?)?;
        let geometry = decode_geometry_lod0(&local)?;
        println!("local='{local_path}' meshes={}", geometry.meshes.len());
        for mesh in &geometry.meshes {
            let Some(skin) = &mesh.skin else { continue };
            let mut mesh_matches = 0_u64;
            for (vertex, skin) in mesh.vertices.iter().zip(skin) {
                let Some(local_joint) = dominant_joint(skin.joints, skin.weights, skin.joints_extra, skin.weights_extra) else { continue };
                *local_joint_totals.entry(local_joint).or_default() += 1;
                let Some(common_candidates) = common_positions.get(&key(vertex.position, quantum)) else {
                    unmatched += 1;
                    continue;
                };
                let mut common_joints = common_candidates.iter().map(|(joint, _, _)| *joint).collect::<Vec<_>>();
                common_joints.sort_unstable();
                common_joints.dedup();
                if common_joints.len() != 1 {
                    ambiguous_matches += 1;
                    continue;
                }
                let common_joint = common_joints[0];
                exact_matches += 1;
                mesh_matches += 1;
                *pairs.entry((common_joint, local_joint)).or_default() += 1;
                *common_joint_totals.entry(common_joint).or_default() += 1;
            }
            println!("  mesh='{}' vertices={} matched={mesh_matches}", mesh.name, mesh.vertices.len());
        }
    }

    println!("summary exact_matches={exact_matches} ambiguous={ambiguous_matches} unmatched={unmatched}");
    println!("-- common_slot -> local_joint evidence --");
    let mut rows = pairs.into_iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for ((common_joint, local_joint), count) in rows {
        let local_total = local_joint_totals.get(&local_joint).copied().unwrap_or(0);
        let common_total = common_joint_totals.get(&common_joint).copied().unwrap_or(0);
        println!(
            "common={common_joint:2} local={local_joint:2} matches={count:5} local_coverage={:.3} common_share={:.3}",
            count as f64 / local_total.max(1) as f64,
            count as f64 / common_total.max(1) as f64,
        );
    }
    Ok(())
}
