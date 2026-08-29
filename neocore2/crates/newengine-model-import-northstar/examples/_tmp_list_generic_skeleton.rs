use newengine_model_import_northstar::{decode_skeleton_with_profile, PakFile, SkeletonProfile};
use std::{env, fs};
fn main() -> Result<(), String> {
    let path = env::args().nth(1).ok_or("path")?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let pak = PakFile::parse(bytes)?;
    let sk = decode_skeleton_with_profile(&pak, SkeletonProfile::Generic)?;
    println!("joints={}", sk.joints.len());
    for (i, j) in sk.joints.iter().enumerate() {
        println!(
            "{}\t{}\t{:?}\t{:?}\t{:?}\t{:?}",
            i, j.name, j.parent_index, j.position_ls, j.rotation_ls, j.scale_ls
        );
    }
    Ok(())
}
