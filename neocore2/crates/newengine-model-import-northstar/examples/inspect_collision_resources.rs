use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| format!("read {source}: {e}"))?)?;
        println!("PAK {source}");
        for (index, resource) in pak.resources().iter().enumerate() {
            if resource.kind != "COLLISION_DATA_HAVOK_BG" {
                continue;
            }
            let payload = pak.resource_payload(resource)?;
            println!(
                "COLLISION index={index} resource=0x{:x} payload=0x{:x} name='{}'",
                resource.absolute_offset, payload, resource.name
            );
            for off in (0..128usize).step_by(8) {
                let field = payload + off;
                let lo = pak.read_u32(field).unwrap_or(0);
                let hi = pak.read_u32(field + 4).unwrap_or(0);
                let resolved = pak.resolve_pointer(field).ok().flatten();
                let annotation = resolved
                    .and_then(|at| {
                        let bytes = pak.slice(at, 8).ok()?;
                        Some(format!(" ->0x{at:x} bytes={:02x?}", bytes))
                    })
                    .unwrap_or_default();
                println!("  +{off:03} raw=0x{hi:08x}{lo:08x}{annotation}");
            }
        }
    }
    Ok(())
}
