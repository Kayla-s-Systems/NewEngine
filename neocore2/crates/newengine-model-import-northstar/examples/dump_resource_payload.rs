use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args.next().ok_or("pak required")?;
    let kind = args.next().ok_or("resource kind required")?;
    let name_filter = args.next().unwrap_or_default();
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    for (index, resource) in pak.resources().iter().enumerate() {
        if resource.kind != kind
            || (!name_filter.is_empty() && !resource.name.contains(&name_filter))
        {
            continue;
        }
        let payload = pak.resource_payload(resource)?;
        println!(
            "RESOURCE {index} kind='{}' name='{}' absolute=0x{:x} payload=0x{:x}",
            resource.kind, resource.name, resource.absolute_offset, payload
        );
        for off in (0..256usize).step_by(8) {
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
                    if !s.is_empty()
                        && s.len() < 180
                        && s.bytes().all(|b| (0x20..0x7f).contains(&b))
                    {
                        note.push_str(&format!(" str={s:?}"));
                    }
                }
            }
            println!("  +{off:03} raw=0x{hi:08x}{lo:08x} lo={lo} hi={hi}{note}");
        }
    }
    Ok(())
}
