use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args
        .next()
        .ok_or("usage: probe_resource_layout <pak> <kind> [bytes]")?;
    let kind = args.next().ok_or("resource kind required")?;
    let limit: usize = args
        .next()
        .as_deref()
        .unwrap_or("512")
        .parse()
        .map_err(|_| "invalid byte count")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let res = pak
        .resource(&kind)
        .ok_or_else(|| format!("missing resource '{kind}'"))?;
    let base = pak.resource_payload(res)?;
    println!("PACKAGE {source} RESOURCE {kind} base=0x{base:x}");
    for off in (0..limit).step_by(4) {
        let at = base + off;
        let u = pak.read_u32(at).unwrap_or_default();
        let f = pak.read_f32(at).unwrap_or(f32::NAN);
        let mut ptr = String::new();
        if off % 8 == 0 {
            if let Ok(Some(p)) = pak.resolve_pointer(at) {
                let s = pak
                    .string_at(p)
                    .ok()
                    .filter(|s| s.bytes().all(|b| b.is_ascii_graphic() || b == b' '));
                ptr = format!(" ptr=0x{p:x} str={s:?}");
            }
        }
        println!("+0x{off:04x} u={u:10} f={f:14.6}{ptr}");
    }
    Ok(())
}
