use newengine_model_import_northstar::{decode_skeleton, PakFile};
use std::{collections::HashMap, env, fs};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let geometry = args.next().ok_or("geometry pak required")?;
    let skeleton_path = args.next().ok_or("skeleton pak required")?;
    let pak = PakFile::parse(fs::read(&geometry).map_err(|e| e.to_string())?)?;
    let skeleton_pak = PakFile::parse(fs::read(&skeleton_path).map_err(|e| e.to_string())?)?;
    let skeleton = decode_skeleton(&skeleton_pak)?;
    let by_tag = skeleton
        .joints
        .iter()
        .map(|joint| (joint.tag, joint.name.as_str()))
        .collect::<HashMap<_, _>>();

    for stride in [4usize, 8, 12, 16] {
        let mut runs = Vec::<(usize, usize)>::new();
        for phase in 0..4usize {
            let mut at = phase;
            let mut run_start = 0usize;
            let mut run_len = 0usize;
            while at + 4 <= pak.bytes().len() {
                let value = u32::from_le_bytes(pak.bytes()[at..at + 4].try_into().unwrap());
                if by_tag.contains_key(&value) {
                    if run_len == 0 {
                        run_start = at;
                    }
                    run_len += 1;
                } else {
                    if run_len >= 4 {
                        runs.push((run_start, run_len));
                    }
                    run_len = 0;
                }
                at += stride;
            }
            if run_len >= 4 {
                runs.push((run_start, run_len));
            }
        }
        runs.sort_by_key(|(_, len)| std::cmp::Reverse(*len));
        println!("STRIDE {stride} runs={}", runs.len());
        for &(start, len) in runs.iter().take(16) {
            print!("  at=0x{start:x} len={len}:");
            for i in 0..len.min(16) {
                let at = start + i * stride;
                let value = u32::from_le_bytes(pak.bytes()[at..at + 4].try_into().unwrap());
                print!(" {value}:{}", by_tag.get(&value).copied().unwrap_or("?"));
            }
            println!();
        }
    }
    Ok(())
}
