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
    let source = env::args()
        .nth(1)
        .ok_or("usage: dump_submesh_record PAK [INDEX]")?;
    let wanted = env::args().nth(2).and_then(|s| s.parse::<usize>().ok());
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let r = pak.resource("GEOMETRY_1").ok_or("no GEOMETRY_1")?;
    let p = pak.resource_payload(r)?;
    let count = pak.read_u32(p + 8)? as usize;
    let table = pak.resolve_pointer(p + 40)?.ok_or("no submesh table")?;
    let stride = if score_stride(&pak, table, count, 192) >= score_stride(&pak, table, count, 176) {
        192
    } else {
        176
    };
    println!("PAK {source} payload=0x{p:x} table=0x{table:x} count={count} stride={stride}");
    for i in 0..count {
        if wanted.is_some_and(|w| w != i) {
            continue;
        }
        let sub = table + i * stride;
        let name = pak
            .resolve_pointer(sub + 32)?
            .map(|x| pak.string_at(x))
            .transpose()?
            .unwrap_or_default();
        if wanted.is_none()
            && !(name.contains("LODShape0") || name.to_ascii_lowercase().contains("lod0"))
        {
            continue;
        }
        println!("\nSUB {i} at=0x{sub:x} name='{name}'");
        for off in (0..stride).step_by(8) {
            let lo = pak.read_u32(sub + off)?;
            let hi = pak.read_u32(sub + off + 4)?;
            let ptr = pak.resolve_pointer(sub + off).ok().flatten();
            let mut annotation = String::new();
            if let Some(ptr) = ptr {
                annotation.push_str(&format!(" ptr=0x{ptr:x}"));
                if let Ok(s) = pak.string_at(ptr) {
                    if !s.is_empty()
                        && s.len() < 220
                        && s.chars().all(|c| !c.is_control() || c == '\t')
                    {
                        annotation.push_str(&format!(" str={s:?}"));
                    }
                }
            }
            println!("  +{off:03} raw=0x{hi:08x}{lo:08x} lo={lo:10} hi={hi:10}{annotation}");
        }
    }
    Ok(())
}
