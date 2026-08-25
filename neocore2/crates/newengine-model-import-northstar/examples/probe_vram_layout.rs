use newengine_model_import_northstar::PakFile;
use std::{env, fs};
fn ascii(bytes: &[u8], base: usize) {
    let mut s = None;
    for (i, &b) in bytes.iter().enumerate() {
        let ok = b.is_ascii_graphic() || b == b' ';
        if ok {
            if s.is_none() {
                s = Some(i)
            }
        } else if let Some(a) = s.take() {
            if i - a >= 4 {
                println!(
                    "    str +0x{:x}: {}",
                    a,
                    String::from_utf8_lossy(&bytes[a..i])
                );
            }
        }
    }
    if let Some(a) = s {
        if bytes.len() - a >= 4 {
            println!(
                "    str +0x{:x}: {}",
                a,
                String::from_utf8_lossy(&bytes[a..])
            );
        }
    }
    let _ = base;
}
fn main() -> Result<(), String> {
    for src in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&src).map_err(|e| e.to_string())?)?;
        println!("PAK {src}");
        for (idx, r) in pak
            .resources()
            .iter()
            .filter(|r| r.kind == "VRAM_DESC")
            .take(8)
            .enumerate()
        {
            let a = r.absolute_offset;
            let p = pak.resource_payload(r)?;
            println!(" #{idx} abs=0x{a:x} payload=0x{p:x} name='{}'", r.name);
            let start = a;
            let len = 256usize.min(pak.bytes().len() - start);
            let b = pak.slice(start, len)?;
            for row in 0..(len + 15) / 16 {
                let st = row * 16;
                let en = (st + 16).min(len);
                print!("   +{:03x}: ", st);
                for x in &b[st..en] {
                    print!("{:02x} ", x);
                }
                println!();
            }
            ascii(b, start);
            for off in (0..160).step_by(8) {
                if let Ok(v) = pak.read_u64(p + off) {
                    if v > 0 && v < 2_000_000 {
                        if let Ok(Some(t)) = pak.resolve_pointer(p + off) {
                            println!(
                                "    fixup payload+0x{off:x}: rel=0x{v:x} -> 0x{t:x} str={:?}",
                                pak.string_at(t).ok()
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
