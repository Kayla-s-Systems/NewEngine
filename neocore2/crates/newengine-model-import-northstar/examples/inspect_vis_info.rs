use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn dump_words(pak: &PakFile, at: usize, words: usize) -> String {
    (0..words)
        .map(|i| {
            pak.read_u32(at + i * 4)
                .map(|v| format!("{v:08x}"))
                .unwrap_or_else(|_| "????????".into())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args
        .next()
        .ok_or("usage: inspect_vis_info <pak> [records]")?;
    let limit: usize = args
        .next()
        .as_deref()
        .unwrap_or("12")
        .parse()
        .map_err(|_| "invalid record count")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let res = pak.resource("VIS_INFO_1").ok_or("VIS_INFO_1 missing")?;
    let base = pak.resource_payload(res)?;
    let version = pak.read_u32(base)?;
    let count = pak.read_u32(base + 4)? as usize;
    let table = pak
        .resolve_pointer(base + 0x10)?
        .ok_or("VIS_INFO_1 record table pointer missing")?;
    println!("VIS_INFO package='{source}' base=0x{base:x} version={version} count={count} table=0x{table:x}");
    for index in 0..count.min(limit) {
        let rec = table + index * 0x48;
        let kind = pak.read_u32(rec)?;
        let flags = pak.read_u32(rec + 4)?;
        let p08 = pak.resolve_pointer(rec + 0x08).ok().flatten();
        let p10 = pak.resolve_pointer(rec + 0x10).ok().flatten();
        let b = [
            pak.read_f32(rec + 0x18)?,
            pak.read_f32(rec + 0x1c)?,
            pak.read_f32(rec + 0x20)?,
            pak.read_f32(rec + 0x24)?,
            pak.read_f32(rec + 0x28)?,
            pak.read_f32(rec + 0x2c)?,
        ];
        let u30 = pak.read_u32(rec + 0x30)?;
        let u34 = pak.read_u32(rec + 0x34)?;
        let p38 = pak.resolve_pointer(rec + 0x38).ok().flatten();
        let id = pak.read_u32(rec + 0x40)?;
        let u44 = pak.read_u32(rec + 0x44)?;
        println!("REC {index:04} kind={kind} flags={flags} id={id} u30={u30} u34={u34} u44={u44} bounds=[{:.3},{:.3},{:.3}]..[{:.3},{:.3},{:.3}]", b[0],b[1],b[2],b[3],b[4],b[5]);
        for (label, ptr) in [("p08", p08), ("p10", p10), ("p38", p38)] {
            if let Some(ptr) = ptr {
                let string = pak.string_at(ptr).ok().filter(|s| {
                    !s.is_empty() && s.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
                });
                println!(
                    "  {label}=0x{ptr:x} words={} str={string:?}",
                    dump_words(&pak, ptr, 12)
                );
            } else {
                println!("  {label}=null");
            }
        }
    }
    Ok(())
}
