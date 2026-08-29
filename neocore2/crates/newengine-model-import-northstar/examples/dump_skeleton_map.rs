use newengine_model_import_northstar::{decode_skeleton, PakFile};
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: dump_skeleton_map SKEL_PAK")?,
    );
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let pak = PakFile::parse(bytes)?;
    let skel = decode_skeleton(&pak)?;
    println!("JOINTS\t{}\t{}", skel.joints.len(), skel.name);
    for j in &skel.joints {
        let parent = j
            .parent_index
            .map(|i| i.to_string())
            .unwrap_or_else(|| "-1".to_owned());
        println!("{}\t{}\t{}", j.index, parent, j.name);
    }
    Ok(())
}
