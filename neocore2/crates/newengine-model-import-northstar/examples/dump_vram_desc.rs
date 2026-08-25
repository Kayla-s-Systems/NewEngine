use newengine_model_import_northstar::PakFile;
use std::{env, fs};
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        let b = pak.bytes();
        let pc = u32le(b, 16) as usize;
        let pt = u32le(b, 20) as usize;
        let lo = u32le(b, pt + (pc - 1) * 12) as usize;
        let ls = u32le(b, pt + (pc - 1) * 12 + 4) as usize;
        let vrbase = lo + ls;
        println!("PAK {source} bytes={} vrbase=0x{vrbase:x}", b.len());
        let mut n = 0;
        for r in pak.resources().iter().filter(|r| r.kind == "VRAM_DESC") {
            let p = pak.resource_payload(r)?;
            if p + 72 >= b.len() {
                continue;
            }
            let po = pak.read_u32(p + 8)? as usize;
            let vs = pak.read_u32(p + 16)? as usize;
            let ty = pak.read_u32(p + 36)?;
            let dx = pak.read_u32(p + 40)?;
            let mip = pak.read_u32(p + 48)?;
            let w = pak.read_u32(p + 52)?;
            let h = pak.read_u32(p + 56)?;
            let sf = pak.read_u64(p + 64)?;
            let path = pak
                .resolve_pointer(p + 72)?
                .and_then(|x| pak.string_at(x).ok())
                .unwrap_or_default();
            let data = vrbase.saturating_add(po);
            let ok = data.checked_add(vs).is_some_and(|e| e <= b.len());
            let low = path.to_ascii_lowercase();
            if low.contains("muzzle")
                || low.contains("spark")
                || low.contains("tracer")
                || low.contains("dust")
                || low.contains("bullet")
                || low.contains("sphere")
                || low.contains("smoke")
                || low.contains("fire")
            {
                println!("  #{n:03} dx=0x{dx:x} {w}x{h} mips={mip} type={ty} bytes={vs} off=0x{po:x} abs=0x{data:x} ok={ok} flags=0x{sf:x} path='{path}'");
            }
            n += 1;
        }
        println!("VRAM count={n}");
    }
    Ok(())
}
