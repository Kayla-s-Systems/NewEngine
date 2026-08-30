use newengine_model_import_northstar::PakFile;
use std::{env, fs};
fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        let Some(res) = pak.resource("LEVEL_INFO_3") else {
            println!("NO_LEVEL_INFO {source}");
            continue;
        };
        let base = pak.resource_payload(res)?;
        println!("PACKAGE {source} LEVEL_INFO base=0x{base:x}");
        for off in (0..512usize).step_by(4) {
            let at = base + off;
            let u = pak.read_u32(at).unwrap_or(0);
            let f = pak.read_f32(at).unwrap_or(f32::NAN);
            let ptr = if off % 8 == 0 {
                pak.resolve_pointer(at).ok().flatten()
            } else {
                None
            };
            let ps = ptr
                .and_then(|p| pak.string_at(p).ok())
                .filter(|s| !s.is_empty() && s.len() < 180);
            if u != 0 || f != 0.0 || ptr.is_some() {
                println!(
                    "+0x{off:03x} u={u:10} f={f:14.6} ptr={} str={:?}",
                    ptr.map(|p| format!("0x{p:x}")).unwrap_or_default(),
                    ps
                );
            }
        }
    }
    Ok(())
}
