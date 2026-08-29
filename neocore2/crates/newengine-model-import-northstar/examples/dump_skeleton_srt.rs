use newengine_model_import_northstar::{decode_skeleton, PakFile};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: dump_skeleton_srt SKEL_PAK")?,
    );
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pak = PakFile::parse(bytes)?;
    let skel = decode_skeleton(&pak)?;
    println!("JOINTS\t{}\t{}", skel.joints.len(), skel.name);
    for j in &skel.joints {
        let p = j
            .parent_index
            .map(|i| i.to_string())
            .unwrap_or_else(|| "-1".to_owned());
        println!(
            "{}\t{}\t{}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}",
            j.index,
            p,
            j.name,
            j.position_ls[0],
            j.position_ls[1],
            j.position_ls[2],
            j.rotation_ls[0],
            j.rotation_ls[1],
            j.rotation_ls[2],
            j.rotation_ls[3],
            j.scale_ls[0],
            j.scale_ls[1],
            j.scale_ls[2]
        );
    }
    Ok(())
}
