use std::{env, fs, path::PathBuf};
use newengine_model_import_northstar::{decode_skeleton, PakFile};

fn main() -> Result<(), String> {
    let path = PathBuf::from(env::args().nth(1).ok_or("usage: dump_facial_hierarchy SKELETON.pak")?);
    let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let pak = PakFile::parse(bytes)?;
    let skeleton = decode_skeleton(&pak)?;
    let headb = skeleton.joints.iter().position(|j| j.name == "headb").ok_or("headb missing")?;
    for (index, joint) in skeleton.joints.iter().enumerate() {
        let lower = joint.name.to_ascii_lowercase();
        if !(lower.contains("eye") || lower.contains("lid") || lower.contains("brow") || lower.contains("face")) {
            continue;
        }
        let mut cursor = Some(index);
        let mut under_headb = false;
        let mut depth = 0usize;
        while let Some(current) = cursor {
            if current == headb { under_headb = true; break; }
            cursor = skeleton.joints[current].parent_index.map(|p| p as usize);
            depth += 1;
            if depth > skeleton.joints.len() { break; }
        }
        println!("FACIAL index={} name='{}' parent={:?} parent_name={:?} under_headb={}", index, joint.name, joint.parent_index, joint.parent_index.map(|p| skeleton.joints[p as usize].name.as_str()), under_headb);
    }
    Ok(())
}
