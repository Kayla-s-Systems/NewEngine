use newengine_model_import_northstar::{decode_geometry_lod0, decode_skeleton, PakFile};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let skeleton_path = PathBuf::from(
        args.next()
            .ok_or("usage: diagnose_ellie_head_hair SKELETON.pak GEOMETRY.pak...")?,
    );
    let skeleton = {
        let bytes = fs::read(&skeleton_path)
            .map_err(|e| format!("read {}: {e}", skeleton_path.display()))?;
        let pak = PakFile::parse(bytes)?;
        decode_skeleton(&pak)?
    };
    println!("SKELETON joints={}", skeleton.joints.len());
    for path in args.map(PathBuf::from) {
        let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let pak = PakFile::parse(bytes)?;
        let geo = decode_geometry_lod0(&pak)?;
        println!("PACKAGE {} meshes={}", path.display(), geo.meshes.len());
        for mesh in geo.meshes {
            let Some(skin) = mesh.skin.as_deref() else {
                continue;
            };
            let mut weights = BTreeMap::<u16, f64>::new();
            let mut weighted_vertices = 0usize;
            for vertex in skin {
                weighted_vertices += 1;
                for (&joint, &weight) in vertex
                    .joints
                    .iter()
                    .chain(vertex.joints_extra.iter())
                    .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
                {
                    if weight > 0.0 {
                        *weights.entry(joint).or_default() += weight as f64;
                    }
                }
            }
            let mut ranked = weights.into_iter().collect::<Vec<_>>();
            ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
            let helper = ranked
                .iter()
                .filter_map(|(joint, weight)| {
                    let j = skeleton.joints.get(*joint as usize)?;
                    j.name.ends_with("_helper").then_some((
                        joint,
                        weight,
                        j.name.as_str(),
                        j.parent_index,
                    ))
                })
                .collect::<Vec<_>>();
            println!("MESH '{}' verts={} max_source_influences={} top8_loss_avg={:.6}% top8_loss_max={:.6}% joints_used={} helpers={}",
                mesh.name,
                weighted_vertices,
                mesh.skin_loss.max_source_influences,
                mesh.skin_loss.average_top8_loss()*100.0,
                mesh.skin_loss.top8_loss_max*100.0,
                ranked.len(), helper.len());
            for (joint, weight) in ranked.iter().take(20) {
                let name = skeleton
                    .joints
                    .get(*joint as usize)
                    .map(|j| j.name.as_str())
                    .unwrap_or("<out-of-range>");
                let parent = skeleton
                    .joints
                    .get(*joint as usize)
                    .and_then(|j| j.parent_index)
                    .and_then(|p| skeleton.joints.get(p as usize))
                    .map(|j| j.name.as_str())
                    .unwrap_or("<root>");
                println!(
                    "  JOINT {:4} weight_sum={:10.4} name='{}' parent='{}'",
                    joint, weight, name, parent
                );
            }
            for (joint, weight, name, parent) in helper.iter().take(20) {
                let parent_name = parent
                    .and_then(|p| skeleton.joints.get(p as usize))
                    .map(|j| j.name.as_str())
                    .unwrap_or("<root>");
                println!(
                    "  HELPER {:4} weight_sum={:10.4} name='{}' parent='{}'",
                    joint, weight, name, parent_name
                );
            }
        }
    }
    Ok(())
}
