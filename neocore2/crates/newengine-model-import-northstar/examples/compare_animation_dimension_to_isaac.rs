use std::collections::BTreeMap;
use std::path::PathBuf;

use newengine_model_import_northstar::{decode_skeleton, PakFile};

fn load(path: &PathBuf) -> Result<PakFile, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    PakFile::parse(bytes)
}

fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err(
            "usage: compare_animation_dimension_to_isaac SOURCE_SKEL ISAAC_SKEL PARTITION"
                .to_owned(),
        );
    }
    let source_path = PathBuf::from(&args[0]);
    let isaac_path = PathBuf::from(&args[1]);
    let partition = args[2]
        .parse::<usize>()
        .map_err(|e| format!("partition: {e}"))?;
    let source_pak = load(&source_path)?;
    let isaac_pak = load(&isaac_path)?;
    let source = decode_skeleton(&source_pak)?;
    let isaac = decode_skeleton(&isaac_pak)?;
    let isaac_by_name = isaac
        .joints
        .iter()
        .map(|j| (j.name.as_str(), j))
        .collect::<BTreeMap<_, _>>();
    println!(
        "source={} joints={} isaac={} joints={} partition={}",
        source.name,
        source.joints.len(),
        isaac.name,
        isaac.joints.len(),
        partition
    );
    if partition > source.joints.len() {
        return Err("partition exceeds source skeleton".to_owned());
    }
    let mut missing = 0usize;
    let mut parent_name_mismatch = 0usize;
    for joint in source.joints.iter().take(partition) {
        let mapped = isaac_by_name.get(joint.name.as_str()).copied();
        match mapped {
            None => {
                missing += 1;
                println!(
                    "MISS src={:03} name='{}' parent={:?}",
                    joint.index, joint.name, joint.parent_index
                );
            }
            Some(target) => {
                let source_parent_name = joint
                    .parent_index
                    .map(|i| source.joints[i as usize].name.as_str());
                let target_parent_name = target
                    .parent_index
                    .map(|i| isaac.joints[i as usize].name.as_str());
                let compatible = source_parent_name == target_parent_name;
                if !compatible {
                    parent_name_mismatch += 1;
                }
                println!("MAP src={:03} -> isaac={:03} name='{}' src_parent={:?} dst_parent={:?} parent_ok={}", joint.index, target.index, joint.name, source_parent_name, target_parent_name, compatible);
            }
        }
    }
    println!(
        "SUMMARY partition={} mapped={} missing={} parent_name_mismatch={}",
        partition,
        partition - missing,
        missing,
        parent_name_mismatch
    );
    Ok(())
}
