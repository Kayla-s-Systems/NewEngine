use std::path::PathBuf;

use newengine_model_import_northstar::{decode_skeleton, PakFile};

fn load(path: &PathBuf) -> Result<PakFile, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    PakFile::parse(bytes)
}

fn main() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 2 {
        return Err(
            "usage: compare_isaac_base_male_skeleton BASE_MALE_SKEL_PAK ISAAC_SKEL_PAK".to_owned(),
        );
    }
    let base_path = PathBuf::from(&args[0]);
    let isaac_path = PathBuf::from(&args[1]);
    let base_pak = load(&base_path)?;
    let isaac_pak = load(&isaac_path)?;
    let base = decode_skeleton(&base_pak)?;
    let isaac = decode_skeleton(&isaac_pak)?;
    println!("base joints={} name={}", base.joints.len(), base.name);
    println!("isaac joints={} name={}", isaac.joints.len(), isaac.name);
    let common = base.joints.len().min(isaac.joints.len());
    let mut name_mismatch = 0usize;
    let mut parent_mismatch = 0usize;
    for index in 0..common {
        let a = &base.joints[index];
        let b = &isaac.joints[index];
        if a.name != b.name {
            name_mismatch += 1;
            println!("NAME {} base='{}' isaac='{}'", index, a.name, b.name);
        }
        if a.parent_index != b.parent_index {
            parent_mismatch += 1;
            println!(
                "PARENT {} '{}' base={:?} isaac={:?}",
                index, a.name, a.parent_index, b.parent_index
            );
        }
    }
    println!(
        "common={} name_mismatch={} parent_mismatch={}",
        common, name_mismatch, parent_mismatch
    );
    for joint in base.joints.iter().take(120) {
        println!(
            "BASE {:03} {:<36} parent={:?}",
            joint.index, joint.name, joint.parent_index
        );
    }
    Ok(())
}
