use newengine_model_import_northstar::PakFile;
use std::{env, fs};

fn score_stride(pak: &PakFile, table: usize, count: usize, stride: usize) -> usize {
    let mut score = 0;
    for i in 0..count.min(64) {
        let sub = table + i * stride;
        if let Ok(Some(ptr)) = pak.resolve_pointer(sub + 32) {
            if let Ok(name) = pak.string_at(ptr) {
                if name.contains("Shape") || name.contains("LOD") {
                    score += 1;
                }
            }
        }
    }
    score
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
        let count_off = if stride == 192 { 136 } else { 128 };
        println!("PAK {source} submeshes={count} stride={stride} score192={s192} score176={s176}");
        for i in 0..count {
            let sub = table + i * stride;
            let name = pak
                .resolve_pointer(sub + 32)?
                .map(|x| pak.string_at(x))
                .transpose()?
                .unwrap_or_default();
            let vc = pak.read_u32(sub + count_off)?;
            let ic = pak.read_u32(sub + count_off + 4)?;
            let sc = pak.read_u32(sub + count_off + 8)? as usize;
            let st = pak.resolve_pointer(sub + 48)?;
            print!("  {i:03} name='{name}' v={vc} i={ic} streams={sc}");
            if let Some(st) = st {
                for j in 0..sc {
                    let at = st + j * 64;
                    let buf = pak.resolve_pointer(at)?.unwrap_or(0);
                    let n = pak.read_u32(at + 8)?;
                    let bs = pak.read_u32(at + 16)?;
                    let k = pak.read_u8(at + 20)?;
                    let sz = [
                        pak.read_u8(at + 24)?,
                        pak.read_u8(at + 25)?,
                        pak.read_u8(at + 26)?,
                        pak.read_u8(at + 27)?,
                    ];
                    print!(" | s{j}:kind={k} n={n} bytes={bs} sizes={sz:?} buf=0x{buf:x}");
                }
            }
            println!();
        }
    }
    Ok(())
}
