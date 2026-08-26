use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn main() -> Result<(), String> {
    let path = env::args().nth(1).ok_or("usage: find_eye_effect PAK")?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let pak = PakFile::parse(bytes)?;
    for (ri, resource) in pak.resources().iter().enumerate() {
        if resource.kind != "EFFECT" { continue; }
        let payload = pak.resource_payload(resource)?;
        let mut hits = Vec::new();
        for off in (0..0x1200usize).step_by(8) {
            if payload + off + 8 > pak.bytes().len() { break; }
            if let Ok(Some(ptr)) = pak.resolve_pointer(payload + off) {
                if let Ok(s) = pak.string_at(ptr) {
                    let low = s.to_ascii_lowercase();
                    if low.contains("objects/characters/ellie/eyes") || low.contains("g_eyewetness") || low.contains("g_sclerabrightness") || low.contains("g_ir") || low.contains("g_pupilsize") {
                        hits.push((off, ptr, s));
                    }
                }
            }
        }
        if !hits.is_empty() {
            println!("EFFECT resource={} absolute=0x{:x} payload=0x{:x} hits={}", ri, resource.absolute_offset, payload, hits.len());
            for (off, ptr, s) in hits { println!("  +0x{off:04x} -> 0x{ptr:x} {s:?}"); }
            for off in (0..0x300usize).step_by(4) {
                if payload + off + 4 > pak.bytes().len() { break; }
                let raw = pak.read_u32(payload + off)?;
                let f = f32::from_bits(raw);
                if f.is_finite() && f.abs() <= 10.0 && (f != 0.0 || raw != 0) {
                    println!("  RAW +0x{off:04x} u32=0x{raw:08x} f32={f:.9}");
                }
            }
        }
    }
    Ok(())
}
