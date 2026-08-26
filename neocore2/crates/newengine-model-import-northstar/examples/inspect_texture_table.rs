use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn main() -> Result<(), String> {
    let source = env::args()
        .nth(1)
        .ok_or("usage: inspect_texture_table PAK")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    for (ri, resource) in pak.resources().iter().enumerate() {
        if resource.kind != "TEXTURE_TABLE" && resource.kind != "VRAM_DESC_TABLE" {
            continue;
        }
        let payload = pak.resource_payload(resource)?;
        println!(
            "RESOURCE {ri} kind={} abs=0x{:x} payload=0x{:x}",
            resource.kind, resource.absolute_offset, payload
        );
        for off in (0..512usize).step_by(8) {
            if payload + off + 8 > pak.bytes().len() {
                break;
            }
            let lo = pak.read_u32(payload + off)?;
            let hi = pak.read_u32(payload + off + 4)?;
            let ptr = pak.resolve_pointer(payload + off).ok().flatten();
            let mut note = String::new();
            if let Some(ptr) = ptr {
                note.push_str(&format!(" ptr=0x{ptr:x}"));
                if let Ok(s) = pak.string_at(ptr) {
                    if !s.is_empty() && s.len() < 220 && s.bytes().all(|b| b >= 0x20 && b < 0x7f) {
                        note.push_str(&format!(" str={s:?}"));
                    }
                }
            }
            println!(" +{off:03} raw=0x{hi:08x}{lo:08x} lo={lo} hi={hi}{note}");
        }
    }
    Ok(())
}
