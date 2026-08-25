use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn score_stride(pak: &PakFile, table: usize, count: usize, stride: usize) -> usize {
    (0..count.min(64))
        .filter(|&i| {
            let sub = table + i * stride;
            pak.resolve_pointer(sub + 32)
                .ok()
                .flatten()
                .and_then(|ptr| pak.string_at(ptr).ok())
                .is_some_and(|name| name.contains("Shape") || name.contains("LOD"))
        })
        .count()
}

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        let r = pak.resource("GEOMETRY_1").ok_or("no GEOMETRY_1")?;
        let p = pak.resource_payload(r)?;
        let count = pak.read_u32(p + 8)? as usize;
        let table = pak.resolve_pointer(p + 40)?.ok_or("no submesh table")?;
        let s192 = score_stride(&pak, table, count, 192);
        let s176 = score_stride(&pak, table, count, 176);
        let stride = if s192 >= s176 { 192 } else { 176 };
        println!("PAK {source} count={count} stride={stride}");
        for i in 0..count {
            let sub = table + i * stride;
            let name = pak
                .resolve_pointer(sub + 32)?
                .map(|x| pak.string_at(x))
                .transpose()?
                .unwrap_or_default();
            if !name.to_ascii_lowercase().contains("lod0") && !name.contains("LODShape0") {
                continue;
            }
            let Some(h) = pak.resolve_pointer(sub + 88)? else {
                continue;
            };
            println!("SUB {i} name='{name}' skin=0x{h:x}");
            for off in (0..=80usize).step_by(8) {
                let lo = pak.read_u32(h + off)?;
                let hi = pak.read_u32(h + off + 4)?;
                let ptr = pak.resolve_pointer(h + off).ok().flatten();
                let mut extra = String::new();
                if let Some(ptr) = ptr {
                    extra.push_str(&format!(" ptr=0x{ptr:x}"));
                    if let Ok(s) = pak.string_at(ptr) {
                        if !s.is_empty() && s.len() < 160 {
                            extra.push_str(&format!(" str={s:?}"));
                        }
                    }
                }
                println!("  +{off:02} lo=0x{lo:08x} ({lo}) hi=0x{hi:08x} ({hi}){extra}");
            }
        }
    }
    Ok(())
}
