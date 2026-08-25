use newengine_model_import_northstar::{decode_skeleton, PakFile};
use std::{env, fs};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("skeleton pak required")?;
    let filters = args
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let pak = PakFile::parse(fs::read(path).map_err(|e| e.to_string())?)?;
    let skeleton = decode_skeleton(&pak)?;
    for (index, joint) in skeleton.joints.iter().enumerate() {
        let lower = joint.name.to_ascii_lowercase();
        if filters.is_empty() || filters.iter().any(|filter| lower.contains(filter)) {
            println!(
                "{index:4} name='{}' parent={:?}",
                joint.name, joint.parent_index
            );
        }
    }
    Ok(())
}
